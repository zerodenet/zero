use std::io;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};
use zero_api::{ApiError, ApiErrorCode, CommandRequest, CommandService, EventFilter};
use zero_engine::Engine;

const MAX_REQUEST_SIZE: usize = 4096;
const MAX_BODY_SIZE: usize = 256 * 1024;
const REQUEST_END: &[u8] = b"\r\n\r\n";

pub struct StatusServerHandle {
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<io::Result<()>>,
}

#[derive(Debug, Clone)]
pub struct StatusServerAuth {
    api_key: String,
}

impl StatusServerAuth {
    pub fn new(api_key: String) -> Self {
        Self { api_key }
    }
}

impl StatusServerHandle {
    pub async fn shutdown(mut self) -> io::Result<()> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }

        self.task.await.expect("status server task should join")
    }
}

pub async fn spawn_status_server(
    engine: Engine,
    listen: &str,
    auth: Option<StatusServerAuth>,
) -> io::Result<StatusServerHandle> {
    let listener = TcpListener::bind(listen).await?;
    let local_addr = listener.local_addr()?;
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    let task =
        tokio::spawn(async move { run_status_server(listener, engine, auth, shutdown_rx).await });

    info!(listen = %local_addr, "local status server ready");

    Ok(StatusServerHandle {
        shutdown: Some(shutdown_tx),
        task,
    })
}

async fn run_status_server(
    listener: TcpListener,
    engine: Engine,
    auth: Option<StatusServerAuth>,
    mut shutdown: oneshot::Receiver<()>,
) -> io::Result<()> {
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            _ = &mut shutdown => break,
            accept_result = listener.accept() => {
                let (stream, remote_addr) = accept_result?;
                let engine = engine.clone();
                let auth = auth.clone();
                connections.spawn(async move {
                    if let Err(error) = handle_connection(stream, engine, auth.as_ref()).await {
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

async fn handle_connection(
    mut stream: TcpStream,
    engine: Engine,
    auth: Option<&StatusServerAuth>,
) -> io::Result<()> {
    let request = read_request(&mut stream).await?;
    let path = normalized_api_path(&request.path);

    if !is_authorized(&request, auth) {
        return write_json_response(
            &mut stream,
            "HTTP/1.1 401 Unauthorized\r\n",
            br#"{"error":"unauthorized"}"#,
        )
        .await;
    }

    let (status_line, body) = match (request.method.as_str(), path) {
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
        ("GET", "/events") => (
            "HTTP/1.1 200 OK\r\n",
            serde_json::to_vec_pretty(&engine.events_snapshot(&EventFilter::default()))
                .map_err(io::Error::other)?,
        ),
        ("POST", "/commands") => match execute_api_command(&engine, &request.body) {
            Ok(body) => ("HTTP/1.1 200 OK\r\n", body),
            Err((status_line, body)) => (status_line, body),
        },
        ("POST", path) => match parse_selector_update_path(path) {
            Some((group_tag, target_tag)) => {
                let command = CommandRequest::PolicySelect(zero_api::PolicySelectCommand {
                    policy_tag: group_tag.to_owned(),
                    target_tag: target_tag.to_owned(),
                });
                match engine.execute(command) {
                    Ok(_) => (
                        "HTTP/1.1 200 OK\r\n",
                        serde_json::to_vec_pretty(&engine.export_config())
                            .map_err(io::Error::other)?,
                    ),
                    Err(error) if error.code == ApiErrorCode::NotFound => api_error_response(error),
                    Err(error)
                        if matches!(
                            error.code,
                            ApiErrorCode::InvalidArgument | ApiErrorCode::Unsupported
                        ) =>
                    {
                        api_error_response(error)
                    }
                    Err(error) => {
                        warn!(error = %error, "selector update failed");
                        api_error_response(error)
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

fn execute_api_command(engine: &Engine, body: &[u8]) -> Result<Vec<u8>, (&'static str, Vec<u8>)> {
    let command = serde_json::from_slice::<CommandRequest>(body).map_err(|error| {
        api_error_response(ApiError {
            code: ApiErrorCode::InvalidArgument,
            message: "invalid command request".to_owned(),
            field_path: None,
            cause: Some(error.to_string()),
        })
    })?;

    engine
        .execute(command)
        .and_then(|response| serde_json::to_vec_pretty(&response).map_err(to_api_internal_error))
        .map_err(api_error_response)
}

fn api_error_response(error: ApiError) -> (&'static str, Vec<u8>) {
    let status_line = match error.code {
        ApiErrorCode::NotFound => "HTTP/1.1 404 Not Found\r\n",
        ApiErrorCode::InvalidArgument => "HTTP/1.1 400 Bad Request\r\n",
        ApiErrorCode::PermissionDenied => "HTTP/1.1 403 Forbidden\r\n",
        ApiErrorCode::FeatureDisabled | ApiErrorCode::Unsupported => {
            "HTTP/1.1 501 Not Implemented\r\n"
        }
        ApiErrorCode::Conflict => "HTTP/1.1 409 Conflict\r\n",
        ApiErrorCode::Internal => "HTTP/1.1 500 Internal Server Error\r\n",
    };

    let body = serde_json::to_vec_pretty(&error).unwrap_or_else(|_| {
        br#"{"code":"internal","message":"failed to serialize error"}"#.to_vec()
    });
    (status_line, body)
}

fn to_api_internal_error(error: serde_json::Error) -> ApiError {
    ApiError {
        code: ApiErrorCode::Internal,
        message: "failed to serialize command response".to_owned(),
        field_path: None,
        cause: Some(error.to_string()),
    }
}

fn normalized_api_path(path: &str) -> &str {
    path.strip_prefix("/api/v1").unwrap_or(path)
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

    let headers = parse_headers(&request);
    let content_length = content_length(&headers)?;
    if content_length > MAX_BODY_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "status request body is too large",
        ));
    }

    let mut body = vec![0_u8; content_length];
    read_request_body(stream, &mut body).await?;

    Ok(HttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body,
    })
}

fn parse_headers(request: &str) -> Vec<(String, String)> {
    request
        .lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect()
}

fn content_length(headers: &[(String, String)]) -> io::Result<usize> {
    let Some((_, value)) = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
    else {
        return Ok(0);
    };

    value
        .parse::<usize>()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid content-length header"))
}

async fn read_request_body(stream: &mut TcpStream, body: &mut [u8]) -> io::Result<()> {
    let mut offset = 0;
    while offset < body.len() {
        let read = stream.read(&mut body[offset..]).await?;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "unexpected EOF while reading status request body",
            ));
        }
        offset += read;
    }
    Ok(())
}

fn is_authorized(request: &HttpRequest, auth: Option<&StatusServerAuth>) -> bool {
    let Some(auth) = auth else {
        return true;
    };
    let bearer = format!("Bearer {}", auth.api_key);

    request.headers.iter().any(|(name, value)| {
        (name.eq_ignore_ascii_case("authorization") && value == &bearer)
            || (name.eq_ignore_ascii_case("x-zero-api-key") && value == &auth.api_key)
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
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}
