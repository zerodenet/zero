use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};
use zero_engine::Engine;

const MAX_REQUEST_SIZE: usize = 4096;
const REQUEST_END: &[u8] = b"\r\n\r\n";

pub struct StatusServerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<io::Result<()>>,
}

impl StatusServerHandle {
    pub async fn shutdown(mut self) -> io::Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        self.task.await.expect("status server task should join")
    }
}

pub async fn spawn_status_server(engine: Engine, listen: &str) -> io::Result<StatusServerHandle> {
    let listener = TcpListener::bind(listen).await?;
    let local_addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let task = tokio::spawn(async move { run_status_server(listener, engine, shutdown_rx).await });

    info!(listen = %local_addr, "local status server ready");

    Ok(StatusServerHandle {
        shutdown: Some(shutdown_tx),
        task,
    })
}

async fn run_status_server(
    listener: TcpListener,
    engine: Engine,
    mut shutdown: oneshot::Receiver<()>,
) -> io::Result<()> {
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accept_result = listener.accept() => {
                let (stream, remote_addr) = accept_result?;
                let engine = engine.clone();
                connections.spawn(async move {
                    if let Err(error) = handle_connection(stream, engine).await {
                        if is_transient_disconnect(&error) {
                            debug!(?remote_addr, error = %error, "status connection closed early");
                        } else {
                            warn!(?remote_addr, error = %error, "status connection failed");
                        }
                    }
                });
            }
            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(Err(error)) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, "status connection task panicked");
                    }
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "status connection task panicked during shutdown");
            }
        }
    }

    info!("local status server stopped");
    Ok(())
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

async fn handle_connection(mut stream: TcpStream, engine: Engine) -> io::Result<()> {
    let request = read_request(&mut stream).await?;
    let (status_line, body) = match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/status") => (
            "HTTP/1.1 200 OK\r\n",
            serde_json::to_vec_pretty(&engine.export_status()).map_err(io::Error::other)?,
        ),
        ("GET", "/config") => (
            "HTTP/1.1 200 OK\r\n",
            serde_json::to_vec_pretty(&engine.export_config()).map_err(io::Error::other)?,
        ),
        ("GET", "/runtime") => (
            "HTTP/1.1 200 OK\r\n",
            serde_json::to_vec_pretty(&engine.export_runtime()).map_err(io::Error::other)?,
        ),
        ("POST", path) => match parse_selector_update_path(path) {
            Some((group_tag, target_tag)) => {
                match engine.set_selector_target(group_tag, target_tag) {
                    Ok(()) => (
                        "HTTP/1.1 200 OK\r\n",
                        serde_json::to_vec_pretty(&engine.export_config())
                            .map_err(io::Error::other)?,
                    ),
                    Err(zero_engine::EngineError::SelectorGroupNotFound { .. }) => (
                        "HTTP/1.1 404 Not Found\r\n",
                        br#"{"error":"selector group not found"}"#.to_vec(),
                    ),
                    Err(
                        zero_engine::EngineError::SelectorGroupTypeMismatch { .. }
                        | zero_engine::EngineError::SelectorTargetNotFound { .. },
                    ) => (
                        "HTTP/1.1 400 Bad Request\r\n",
                        br#"{"error":"invalid selector update"}"#.to_vec(),
                    ),
                    Err(error) => {
                        warn!(error = %error, "selector update failed");
                        (
                            "HTTP/1.1 500 Internal Server Error\r\n",
                            br#"{"error":"selector update failed"}"#.to_vec(),
                        )
                    }
                }
            }
            None => (
                "HTTP/1.1 404 Not Found\r\n",
                br#"{"error":"not found"}"#.to_vec(),
            ),
        },
        _ if request.method == "GET" => (
            "HTTP/1.1 404 Not Found\r\n",
            br#"{"error":"not found"}"#.to_vec(),
        ),
        _ => (
            "HTTP/1.1 405 Method Not Allowed\r\n",
            br#"{"error":"method not allowed"}"#.to_vec(),
        ),
    };

    write_json_response(&mut stream, status_line, &body).await
}

async fn read_request(stream: &mut TcpStream) -> io::Result<HttpRequest> {
    let mut request = Vec::new();

    loop {
        if request.len() >= MAX_REQUEST_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "status request head is too large",
            ));
        }

        let mut byte = [0_u8; 1];
        let read = stream.read(&mut byte).await?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "unexpected EOF while reading status request",
            ));
        }

        request.push(byte[0]);
        if request.ends_with(REQUEST_END) {
            break;
        }
    }

    let request = String::from_utf8(request)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "status request is not utf-8"))?;
    let first_line = request
        .lines()
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request line"))?;
    let mut parts = first_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request method"))?;
    let path = parts
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request path"))?;

    Ok(HttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
    })
}

async fn write_json_response(
    stream: &mut TcpStream,
    status_line: &str,
    body: &[u8],
) -> io::Result<()> {
    let headers = format!(
        "{status_line}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );

    stream.write_all(headers.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.shutdown().await
}

fn parse_selector_update_path(path: &str) -> Option<(&str, &str)> {
    let segments = path.split('/').collect::<Vec<_>>();
    match segments.as_slice() {
        ["", "selectors", group_tag, outbound_tag]
            if !group_tag.is_empty() && !outbound_tag.is_empty() =>
        {
            Some((group_tag, outbound_tag))
        }
        _ => None,
    }
}

struct HttpRequest {
    method: String,
    path: String,
}
