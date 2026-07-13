use std::io;

use http::Request;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use zero_engine::EngineError;

pub(super) fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

pub(super) fn parse_status(buf: &[u8]) -> Option<u16> {
    let head = std::str::from_utf8(buf).ok()?;
    let first_line = head.lines().next()?;
    let parts: Vec<_> = first_line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse().ok()
    } else {
        None
    }
}

pub(super) fn parse_method_and_session(buf: &[u8]) -> Result<(String, String), EngineError> {
    let head = std::str::from_utf8(buf).map_err(|_| {
        EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "split-http: non-UTF-8 headers",
        ))
    })?;
    let first_line = head.lines().next().ok_or_else(|| {
        EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "split-http: empty request",
        ))
    })?;
    let parts: Vec<_> = first_line.split_whitespace().collect();
    if parts.len() < 2 {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "split-http: malformed request line",
        )));
    }
    let method = parts[0].to_string();
    let session_id = head
        .lines()
        .find_map(|line| {
            line.to_lowercase()
                .strip_prefix("x-session-id:")
                .map(|value| value.trim().to_string())
        })
        .unwrap_or_else(|| "0".to_string());
    Ok((method, session_id))
}

pub(super) fn validate_path(buf: &[u8], expected: &str) -> Result<(), EngineError> {
    let head = std::str::from_utf8(buf).map_err(|_| {
        EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "split-http: non-UTF-8 headers",
        ))
    })?;
    let first_line = head.lines().next().ok_or_else(|| {
        EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "split-http: empty request",
        ))
    })?;
    let parts: Vec<_> = first_line.split_whitespace().collect();
    if parts.len() >= 2 && parts[1] != expected {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("split-http: path mismatch, expected {expected}"),
        )));
    }
    Ok(())
}

pub(super) async fn write_get_response<W: AsyncWrite + Unpin>(
    writer: &mut W,
) -> Result<(), EngineError> {
    writer
        .write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
        .await
        .map_err(EngineError::Io)
}

pub(super) fn write_http_request(buf: &mut Vec<u8>, req: &Request<()>) {
    let path = req.uri().path_and_query().map_or("/", |uri| uri.as_str());
    let mut request = String::with_capacity(256);
    request.push_str(&format!("{} {} HTTP/1.1\r\n", req.method().as_str(), path));
    for (name, value) in req.headers() {
        request.push_str(&format!(
            "{}: {}\r\n",
            name.as_str(),
            value.to_str().unwrap_or("")
        ));
    }
    request.push_str("\r\n");
    buf.extend_from_slice(request.as_bytes());
}
