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
    let path = read_path(&mut stream).await?;
    let (status_line, body) = match path.as_str() {
        "/status" => (
            "HTTP/1.1 200 OK\r\n",
            serde_json::to_vec_pretty(&engine.export_status()).map_err(io::Error::other)?,
        ),
        "/config" => (
            "HTTP/1.1 200 OK\r\n",
            serde_json::to_vec_pretty(&engine.export_config()).map_err(io::Error::other)?,
        ),
        "/runtime" => (
            "HTTP/1.1 200 OK\r\n",
            serde_json::to_vec_pretty(&engine.export_runtime()).map_err(io::Error::other)?,
        ),
        _ => (
            "HTTP/1.1 404 Not Found\r\n",
            br#"{"error":"not found"}"#.to_vec(),
        ),
    };

    write_json_response(&mut stream, status_line, &body).await
}

async fn read_path(stream: &mut TcpStream) -> io::Result<String> {
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

    if method != "GET" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "status server only supports GET",
        ));
    }

    Ok(path.to_owned())
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
