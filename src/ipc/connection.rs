//! Shared IPC connection handling logic.
//!
//! This module contains the transport-agnostic request/response loop,
//! command parsing, and utility functions used by both the Unix domain
//! socket and Windows named pipe IPC servers.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::Mutex as TokioMutex;
use zero_api::{
    AuthContext, CommandRequest, CommandService, DiagnosticsDnsLookupCommand,
    DiagnosticsTraceRouteCommand, EventFilter, EventSource, Permission, PolicySelectCommand,
    QueryService, TunStopCommand,
};
use zero_proxy::ProxyHandle;

use super::protocol::{serialize_frame, IpcRequest, IpcResponse};

/// Parse a string method name + JSON params into a typed `CommandRequest`.
pub(crate) fn parse_command(
    method: &str,
    params: &serde_json::Value,
) -> Result<CommandRequest, IpcResponse> {
    match method {
        "policies.select" => {
            let policy_tag = params["policy_tag"]
                .as_str()
                .ok_or_else(|| IpcResponse::error("invalid_argument", "missing policy_tag"))?;
            let target_tag = params["target_tag"]
                .as_str()
                .ok_or_else(|| IpcResponse::error("invalid_argument", "missing target_tag"))?;
            Ok(CommandRequest::PolicySelect(PolicySelectCommand {
                policy_tag: policy_tag.to_owned(),
                target_tag: target_tag.to_owned(),
            }))
        }
        "policies.probe" => {
            let policy_tag = params["policy_tag"]
                .as_str()
                .ok_or_else(|| IpcResponse::error("invalid_argument", "missing policy_tag"))?;
            Ok(CommandRequest::PolicyProbe(zero_api::PolicyProbeCommand {
                policy_tag: policy_tag.to_owned(),
            }))
        }
        "flows.close" => {
            let flow_id = params["flow_id"]
                .as_str()
                .ok_or_else(|| IpcResponse::error("invalid_argument", "missing flow_id"))?;
            Ok(CommandRequest::FlowClose(zero_api::FlowCloseCommand {
                flow_id: flow_id.to_owned(),
            }))
        }
        "config.validate" => Ok(CommandRequest::ConfigValidate(
            zero_api::ConfigValidateCommand {
                config: params.clone(),
            },
        )),
        "config.apply" => Ok(CommandRequest::ConfigApply(zero_api::ConfigApplyCommand {
            config: params.clone(),
        })),
        "diagnostics.probe_target" => {
            let target_tag = params["target_tag"]
                .as_str()
                .ok_or_else(|| IpcResponse::error("invalid_argument", "missing target_tag"))?;
            Ok(CommandRequest::DiagnosticsProbeTarget(
                zero_api::DiagnosticsProbeTargetCommand {
                    target_tag: target_tag.to_owned(),
                },
            ))
        }
        "diagnostics.dns_lookup" => {
            let hostname = params["hostname"]
                .as_str()
                .ok_or_else(|| IpcResponse::error("invalid_argument", "missing hostname"))?;
            Ok(CommandRequest::DiagnosticsDnsLookup(
                DiagnosticsDnsLookupCommand {
                    hostname: hostname.to_owned(),
                },
            ))
        }
        "diagnostics.trace_route" => {
            let target = params["target"]
                .as_str()
                .ok_or_else(|| IpcResponse::error("invalid_argument", "missing target"))?;
            let port = params["port"]
                .as_u64()
                .ok_or_else(|| IpcResponse::error("invalid_argument", "missing port"))?
                as u16;
            let protocol = params["protocol"].as_str().map(|s| s.to_owned());
            Ok(CommandRequest::DiagnosticsTraceRoute(
                DiagnosticsTraceRouteCommand {
                    target: target.to_owned(),
                    port,
                    protocol,
                },
            ))
        }
        "mode.set" => Ok(CommandRequest::ModeSet(
            serde_json::from_value(params.clone())
                .map_err(|e| IpcResponse::error("invalid_argument", e.to_string()))?,
        )),
        "tun.start" => Ok(CommandRequest::TunStart(
            serde_json::from_value(params.clone())
                .map_err(|e| IpcResponse::error("invalid_argument", e.to_string()))?,
        )),
        "tun.stop" => Ok(CommandRequest::TunStop(TunStopCommand)),
        _ => Err(IpcResponse::error(
            "unsupported",
            format!("unknown command method: {method}"),
        )),
    }
}

/// Handle a single IPC connection: read JSON-line frames, dispatch
/// queries/commands/subscriptions, and write responses.
///
/// After a `Subscribe`, the connection stays open for event streaming
/// AND continues to accept `Query` / `Command` / `Ping` frames.
/// Responses echo the request `id` so the client can pair them on a
/// multiplexed connection.
pub(crate) async fn handle_ipc_connection<S>(stream: S, handle: ProxyHandle) -> io::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    let (reader, writer) = tokio::io::split(stream);
    let mut reader = BufReader::new(reader);
    let writer = Arc::new(TokioMutex::new(BufWriter::new(writer)));

    // IPC connections are protected by OS-level file permissions
    // (Unix socket 0o600).  All callers are trusted with admin.
    let auth_ctx = AuthContext {
        subject: Some("ipc-local".to_owned()),
        permissions: vec![
            Permission::Read,
            Permission::Control,
            Permission::Config,
            Permission::Admin,
        ],
    };

    let mut subscribed = false;
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
                write_ipc_response(&mut *writer.lock().await, &resp).await?;
                continue;
            }
        };

        let req_id = request.id();

        match request {
            IpcRequest::Ping { .. } => {
                let resp = IpcResponse::ok("pong").with_id(req_id);
                write_ipc_response(&mut *writer.lock().await, &resp).await?;
            }
            IpcRequest::Query { request, .. } => {
                let result = handle.query(request);
                match result {
                    Ok(resp) => {
                        let value = serde_json::to_value(resp).map_err(io::Error::other)?;
                        let resp = IpcResponse::ok_raw(value).with_id(req_id);
                        write_ipc_response(&mut *writer.lock().await, &resp).await?;
                    }
                    Err(error) => {
                        let resp = IpcResponse::from_api_error(&error).with_id(req_id);
                        write_ipc_response(&mut *writer.lock().await, &resp).await?;
                    }
                }
            }
            IpcRequest::Command { method, params, .. } => {
                let command = parse_command(&method, &params);
                match command {
                    Ok(ref cmd) if !auth_ctx.allows(cmd.required_permission()) => {
                        let error =
                            zero_api::ApiError::permission_denied(cmd.required_permission());
                        let resp = IpcResponse::from_api_error(&error).with_id(req_id);
                        write_ipc_response(&mut *writer.lock().await, &resp).await?;
                    }
                    Ok(cmd) => match handle.execute(cmd) {
                        Ok(resp) => {
                            let value = serde_json::to_value(resp).map_err(io::Error::other)?;
                            let resp = IpcResponse::ok_raw(value).with_id(req_id);
                            write_ipc_response(&mut *writer.lock().await, &resp).await?;
                        }
                        Err(error) => {
                            let resp = IpcResponse::from_api_error(&error).with_id(req_id);
                            write_ipc_response(&mut *writer.lock().await, &resp).await?;
                        }
                    },
                    Err(error) => {
                        let resp = error.with_id(req_id);
                        write_ipc_response(&mut *writer.lock().await, &resp).await?;
                    }
                }
            }
            IpcRequest::Subscribe { events, .. } => {
                if subscribed {
                    let resp = IpcResponse::error(
                        "already_subscribed",
                        "this connection is already subscribed to events",
                    )
                    .with_id(req_id);
                    write_ipc_response(&mut *writer.lock().await, &resp).await?;
                    continue;
                }
                subscribed = true;

                let resp = IpcResponse::ok("subscribed").with_id(req_id);
                write_ipc_response(&mut *writer.lock().await, &resp).await?;

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

                        // Spawn event writer task so the main loop can
                        // continue reading query/command/ping frames.
                        let event_writer = writer.clone();
                        let event_task = tokio::spawn(async move {
                            loop {
                                match event_rx.recv_timeout(Duration::from_secs(30)) {
                                    Ok(value) => {
                                        let frame =
                                            serialize_frame(&value).map_err(io::Error::other)?;
                                        let mut w = event_writer.lock().await;
                                        w.write_all(&frame).await?;
                                        w.flush().await?;
                                    }
                                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                                        // heartbeat
                                        let mut w = event_writer.lock().await;
                                        w.write_all(b":\n").await?;
                                        w.flush().await?;
                                    }
                                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                                        break;
                                    }
                                }
                            }
                            io::Result::Ok(())
                        });

                        // Don't break — continue the loop for subsequent requests.
                        // The event_task runs concurrently.
                        // When the client disconnects, read_line returns 0 → break,
                        // event_task's write fails → task exits, blocking task aborted.
                        let _ = (event_task, blocking);
                    }
                    Err(error) => {
                        let resp = IpcResponse::from_api_error(&error).with_id(req_id);
                        write_ipc_response(&mut *writer.lock().await, &resp).await?;
                        subscribed = false;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Write a serialized IPC response frame to the transport.
pub(crate) async fn write_ipc_response(
    writer: &mut (impl AsyncWriteExt + Unpin),
    response: &IpcResponse,
) -> io::Result<()> {
    let frame = serialize_frame(response).map_err(io::Error::other)?;
    writer.write_all(&frame).await?;
    Ok(())
}

/// Return true if the I/O error is from a routine client disconnect.
pub(crate) fn is_transient_disconnect(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::UnexpectedEof
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::BrokenPipe
            | io::ErrorKind::NotConnected
    )
}
