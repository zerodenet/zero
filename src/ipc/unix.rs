pub mod client;
pub mod protocol;

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};
use zero_api::{
    CommandRequest, CommandService, EventFilter, EventSource, PolicySelectCommand, QueryService,
};
use zero_engine::EngineHandle;

use protocol::{serialize_frame, IpcRequest, IpcResponse};

/// Default control socket path relative to $HOME.
pub const DEFAULT_SOCKET_REL_PATH: &str = ".zero/control.sock";

pub struct IpcServerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<io::Result<()>>,
    socket_path: PathBuf,
}

impl IpcServerHandle {
    pub async fn shutdown(mut self) -> io::Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        let result = self.task.await.expect("ipc server task should join");
        let _ = std::fs::remove_file(&self.socket_path);
        result
    }
}

/// Compute the default socket path: `$HOME/.zero/control.sock`.
pub fn default_socket_path() -> Option<PathBuf> {
    let home = dirs_home()?;
    Some(home.join(DEFAULT_SOCKET_REL_PATH))
}

/// Resolve the socket path: explicit path, or config path, or default.
pub fn resolve_socket_path(explicit: Option<&str>) -> io::Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(PathBuf::from(path));
    }
    default_socket_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "cannot determine home directory for control socket",
        )
    })
}

/// Spawn the IPC server on a Unix domain socket.
///
/// The socket file is created with `0o600` permissions so only the owning
/// user can connect.  If the socket file already exists it is removed first.
pub async fn spawn_ipc_server(
    engine_handle: EngineHandle,
    socket_path: &Path,
) -> io::Result<IpcServerHandle> {
    if socket_path.exists() {
        std::fs::remove_file(socket_path)?;
    }

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let listener = UnixListener::bind(socket_path)?;
    let metadata = socket_path.metadata()?;
    let mut perms = metadata.permissions();
    perms.set_mode(0o600);
    std::fs::set_permissions(socket_path, perms)?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let path_for_handle = socket_path.to_path_buf();

    let task = tokio::spawn(async move { run_ipc_server(listener, engine_handle, shutdown_rx).await });

    info!(socket = %socket_path.display(), "ipc server ready");

    Ok(IpcServerHandle {
        shutdown: Some(shutdown_tx),
        task,
        socket_path: path_for_handle,
    })
}

async fn run_ipc_server(
    listener: UnixListener,
    handle: EngineHandle,
    mut shutdown: oneshot::Receiver<()>,
) -> io::Result<()> {
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accept_result = listener.accept() => {
                let (stream, peer_addr) = accept_result?;
                let handle = handle.clone();
                connections.spawn(async move {
                    if let Err(error) = handle_ipc_connection(stream, handle).await {
                        if is_transient_disconnect(&error) {
                            debug!(?peer_addr, error = %error, "ipc connection closed early");
                        } else {
                            warn!(?peer_addr, error = %error, "ipc connection failed");
                        }
                    }
                });
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, "ipc connection task panicked");
                    }
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "ipc connection task panicked during shutdown");
            }
        }
    }

    info!("ipc server stopped");
    Ok(())
}

async fn handle_ipc_connection(stream: UnixStream, handle: EngineHandle) -> io::Result<()> {
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut writer = BufWriter::new(writer);

    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }

        let request: IpcRequest = match serde_json::from_str(line.trim()) {
            Ok(req) => req,
            Err(_) => {
                let resp = IpcResponse::error("invalid_argument", "invalid request frame");
                write_ipc_response(&mut writer, &resp).await?;
                continue;
            }
        };

        match request {
            IpcRequest::Ping => {
                write_ipc_response(&mut writer, &IpcResponse::ok("pong")).await?;
            }
            IpcRequest::Query { request } => {
                let result = handle.query(request);
                match result {
                    Ok(resp) => {
                        let value = serde_json::to_value(resp).map_err(io::Error::other)?;
                        write_ipc_response(&mut writer, &IpcResponse::ok_raw(value)).await?;
                    }
                    Err(error) => {
                        write_ipc_response(
                            &mut writer,
                            &IpcResponse::from_api_error(&error),
                        )
                        .await?;
                    }
                }
            }
            IpcRequest::Command { method, params } => {
                let command = parse_command(&method, &params);
                match command {
                    Ok(cmd) => match handle.execute(cmd) {
                        Ok(resp) => {
                            let value =
                                serde_json::to_value(resp).map_err(io::Error::other)?;
                            write_ipc_response(&mut writer, &IpcResponse::ok_raw(value))
                                .await?;
                        }
                        Err(error) => {
                            write_ipc_response(
                                &mut writer,
                                &IpcResponse::from_api_error(&error),
                            )
                            .await?;
                        }
                    },
                    Err(error) => {
                        write_ipc_response(&mut writer, &error).await?;
                    }
                }
            }
            IpcRequest::Subscribe { events } => {
                write_ipc_response(&mut writer, &IpcResponse::ok("subscribed")).await?;
                writer.flush().await?;

                let mut filter = EventFilter::default();
                if let Some(types) = events {
                    filter.event_types = types;
                }

                match handle.subscribe(filter) {
                    Ok(subscriber) => {
                        let (event_tx, event_rx) = std::sync::mpsc::channel();

                        let blocking = tokio::task::spawn_blocking(move || {
                            while let Some(event) = subscriber.recv() {
                                let value =
                                    serde_json::to_value(&protocol::IpcEvent::Event {
                                        event_type: event.event_type.clone(),
                                        event_id: event.event_id.clone(),
                                        occurred_at_unix_ms: event.occurred_at_unix_ms,
                                        payload: event.payload.clone(),
                                    });
                                if let Ok(value) = value {
                                    if event_tx.send(value).is_err() {
                                        break;
                                    }
                                }
                            }
                        });

                        loop {
                            match event_rx.recv_timeout(Duration::from_secs(30)) {
                                Ok(value) => {
                                    let frame =
                                        serialize_frame(&value).map_err(io::Error::other)?;
                                    writer.write_all(&frame).await?;
                                    writer.flush().await?;
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                    // heartbeat
                                    writer.write_all(b":\n").await?;
                                    writer.flush().await?;
                                }
                                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                    break;
                                }
                            }
                        }

                        blocking.abort();
                    }
                    Err(error) => {
                        write_ipc_response(
                            &mut writer,
                            &IpcResponse::from_api_error(&error),
                        )
                        .await?;
                    }
                }

                break;
            }
        }

        writer.flush().await?;
    }

    Ok(())
}

async fn write_ipc_response(
    writer: &mut BufWriter<impl AsyncWriteExt + Unpin>,
    response: &IpcResponse,
) -> io::Result<()> {
    let frame = serialize_frame(response).map_err(io::Error::other)?;
    writer.write_all(&frame).await?;
    Ok(())
}

fn parse_command(method: &str, params: &serde_json::Value) -> Result<CommandRequest, IpcResponse> {
    match method {
        "policies.select" => {
            let policy_tag = params["policy_tag"].as_str().ok_or_else(|| {
                IpcResponse::error("invalid_argument", "missing policy_tag")
            })?;
            let target_tag = params["target_tag"].as_str().ok_or_else(|| {
                IpcResponse::error("invalid_argument", "missing target_tag")
            })?;
            Ok(CommandRequest::PolicySelect(PolicySelectCommand {
                policy_tag: policy_tag.to_owned(),
                target_tag: target_tag.to_owned(),
            }))
        }
        "policies.probe" => {
            let policy_tag = params["policy_tag"].as_str().ok_or_else(|| {
                IpcResponse::error("invalid_argument", "missing policy_tag")
            })?;
            Ok(CommandRequest::PolicyProbe(
                zero_api::PolicyProbeCommand {
                    policy_tag: policy_tag.to_owned(),
                },
            ))
        }
        "flows.close" => {
            let flow_id = params["flow_id"].as_str().ok_or_else(|| {
                IpcResponse::error("invalid_argument", "missing flow_id")
            })?;
            Ok(CommandRequest::FlowClose(zero_api::FlowCloseCommand {
                flow_id: flow_id.to_owned(),
            }))
        }
        "config.validate" => Ok(CommandRequest::ConfigValidate(
            zero_api::ConfigValidateCommand {
                config: params.clone(),
            },
        )),
        _ => Err(IpcResponse::error(
            "unsupported",
            format!("unknown command method: {method}"),
        )),
    }
}

fn is_transient_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::UnexpectedEof
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::NotConnected
    )
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}
