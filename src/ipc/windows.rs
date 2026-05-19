//! Windows Named Pipe IPC server and client.
//!
//! Named Pipes live in the `\\.\pipe\` namespace — they are not filesystem
//! objects, so the path is always the well-known `\\.\pipe\zero-control`
//! unless overridden via CLI.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};
use zero_api::{
    CommandRequest, CommandService, EventFilter, EventSource, PolicySelectCommand, QueryService,
};
use zero_engine::EngineHandle;

use super::protocol::{serialize_frame, IpcRequest, IpcResponse};

pub const PIPE_NAME: &str = r"\\.\pipe\zero-control";

pub struct IpcServerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<io::Result<()>>,
}

impl IpcServerHandle {
    pub async fn shutdown(mut self) -> io::Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.await.expect("ipc server task should join")
    }
}

pub fn default_socket_path() -> Option<PathBuf> {
    Some(PathBuf::from(PIPE_NAME))
}

pub fn resolve_socket_path(explicit: Option<&str>) -> io::Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(PathBuf::from(path));
    }
    Ok(PathBuf::from(PIPE_NAME))
}

pub async fn spawn_ipc_server(
    engine_handle: EngineHandle,
    pipe_path: &std::path::Path,
) -> io::Result<IpcServerHandle> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let pipe = pipe_path.to_string_lossy().to_string();

    let task = tokio::spawn(async move {
        run_ipc_server(&pipe, engine_handle, shutdown_rx).await
    });

    info!(pipe = %pipe_path.display(), "ipc server ready");

    Ok(IpcServerHandle {
        shutdown: Some(shutdown_tx),
        task,
    })
}

async fn run_ipc_server(
    pipe_name: &str,
    handle: EngineHandle,
    mut shutdown: oneshot::Receiver<()>,
) -> io::Result<()> {
    let mut connections = JoinSet::new();

    loop {
        let mut server = match ServerOptions::new().create(pipe_name) {
            Ok(s) => s,
            Err(e) => {
                error!(pipe = %pipe_name, error = %e, "failed to create named pipe");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };

        tokio::select! {
            _ = &mut shutdown => break,
            result = server.connect() => {
                if let Err(e) = result {
                    warn!(pipe = %pipe_name, error = %e, "named pipe connect failed");
                    continue;
                }

                let handle = handle.clone();
                connections.spawn(async move {
                    if let Err(error) = handle_ipc_connection(server, handle).await {
                        if is_transient_disconnect(&error) {
                            debug!(error = %error, "ipc connection closed early");
                        } else {
                            warn!(error = %error, "ipc connection failed");
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

async fn handle_ipc_connection<S>(stream: S, handle: EngineHandle) -> io::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (reader, writer) = tokio::io::split(stream);
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
                                    serde_json::to_value(&super::protocol::IpcEvent::Event {
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

// ── Client ────────────────────────────────────────────────────────────

use std::io::{BufRead as StdBufRead, BufReader as StdBufReader, Write};

/// Connect to the named pipe and send a single request.
pub fn send_request(pipe_name: &str, request: &IpcRequest) -> io::Result<IpcResponse> {
    let stream = open_pipe_client(pipe_name)?;
    send_frame(&stream, request)?;
    read_response(&stream)
}

/// Connect and stream events.
pub fn stream_events(
    pipe_name: &str,
    request: &IpcRequest,
    mut on_event: impl FnMut(serde_json::Value),
) -> io::Result<()> {
    let stream = open_pipe_client(pipe_name)?;
    send_frame(&stream, request)?;

    let reader = StdBufReader::new(&stream);
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(&line).map_err(io::Error::other)?;
        on_event(value);
    }
    Ok(())
}

fn open_pipe_client(pipe_name: &str) -> io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(pipe_name)
}

fn send_frame(mut stream: &std::fs::File, request: &IpcRequest) -> io::Result<()> {
    let frame = serialize_frame(request).map_err(io::Error::other)?;
    stream.write_all(&frame)?;
    Ok(())
}

fn read_response(stream: &std::fs::File) -> io::Result<IpcResponse> {
    let reader = StdBufReader::new(stream);
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        return serde_json::from_str(&line).map_err(io::Error::other);
    }
    Err(io::Error::new(
        io::ErrorKind::UnexpectedEof,
        "no response from ipc pipe",
    ))
}

// ── Helpers ───────────────────────────────────────────────────────────

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
            Ok(CommandRequest::PolicyProbe(zero_api::PolicyProbeCommand {
                policy_tag: policy_tag.to_owned(),
            }))
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
        "config.apply" => Ok(CommandRequest::ConfigApply(
            zero_api::ConfigApplyCommand {
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
