//! HTTPUpgrade transport 鈥?http_upgrade.rs
//!
//! Lightweight WebSocket alternative: a single HTTP upgrade handshake
//! produces a raw bidirectional stream. No frame headers, no masking.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use std::net::SocketAddr;

use crate::RuntimeError;
use http::{Method, Request};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use zero_platform_tokio::ClientStream;
use zero_traits::{AsyncSocket, HttpUpgradeTransportProfile};

/// Bidirectional stream after HTTP upgrade.
pub struct HttpUpgradeStream<S> {
    inner: S,
}

// 鈹€鈹€ client (outbound) connect 鈹€鈹€

/// Connect via HTTPUpgrade: send GET + Upgrade header, expect 101.
pub async fn connect_http_upgrade<S, TProfile>(
    stream: S,
    config: &TProfile,
) -> Result<HttpUpgradeStream<S>, RuntimeError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    TProfile: HttpUpgradeTransportProfile + ?Sized,
{
    let host = config.host().unwrap_or("localhost");
    let path = config.path();

    let req = Request::builder()
        .method(Method::GET)
        .uri(path)
        .header("Host", host)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(())
        .map_err(|e| RuntimeError::Io(io::Error::other(format!("http-upgrade request: {e}"))))?;

    let mut io = stream;

    // Write the request
    let mut req_bytes = Vec::new();
    write_http_request(&mut req_bytes, &req);
    io.write_all(&req_bytes).await.map_err(RuntimeError::Io)?;

    // Read the response
    let mut buf = vec![0u8; 4096];
    let mut total = 0;
    loop {
        let n = io.read(&mut buf[total..]).await.map_err(RuntimeError::Io)?;
        if n == 0 {
            return Err(RuntimeError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "http-upgrade: unexpected EOF",
            )));
        }
        total += n;
        if find_header_end(&buf[..total]).is_some() {
            let status = parse_status(&buf[..total]).ok_or_else(|| {
                RuntimeError::Io(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "http-upgrade: bad response",
                ))
            })?;
            if status != 101 {
                return Err(RuntimeError::Io(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    format!("http-upgrade: server returned {status}"),
                )));
            }
            // Remaining bytes after headers are data
            // (but typically there's none after 101)
            break;
        }
        if total >= buf.len() {
            return Err(RuntimeError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "http-upgrade: response headers too large",
            )));
        }
    }

    Ok(HttpUpgradeStream { inner: io })
}

// 鈹€鈹€ server (inbound) accept 鈹€鈹€

/// Accept an HTTPUpgrade connection: read upgrade request, respond 101.
pub async fn accept_http_upgrade<S, TProfile>(
    stream: S,
    config: &TProfile,
) -> Result<HttpUpgradeStream<S>, RuntimeError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    TProfile: HttpUpgradeTransportProfile + ?Sized,
{
    let mut io = stream;

    // Read the upgrade request
    let mut buf = vec![0u8; 4096];
    let mut total = 0;
    loop {
        let n = io.read(&mut buf[total..]).await.map_err(RuntimeError::Io)?;
        if n == 0 {
            return Err(RuntimeError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "http-upgrade accept: unexpected EOF",
            )));
        }
        total += n;
        if find_header_end(&buf[..total]).is_some() {
            let req_path = parse_request_path(&buf[..total]);
            let expected = config.path();
            if req_path.as_deref() != Some(expected) {
                return Err(RuntimeError::Io(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    format!("http-upgrade: path mismatch, expected {expected}"),
                )));
            }
            break;
        }
        if total >= buf.len() {
            return Err(RuntimeError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "http-upgrade accept: request headers too large",
            )));
        }
    }

    // Send 101 Switching Protocols
    let resp = "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
    io.write_all(resp.as_bytes())
        .await
        .map_err(RuntimeError::Io)?;

    Ok(HttpUpgradeStream { inner: io })
}

// 鈹€鈹€ AsyncRead / AsyncWrite / AsyncSocket 鈹€鈹€

impl<S> AsyncRead for HttpUpgradeStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<S> AsyncWrite for HttpUpgradeStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<S> AsyncSocket for HttpUpgradeStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    type Error = io::Error;

    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>> + Send + 'a {
        async move { AsyncReadExt::read(self, buf).await }
    }
    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move {
            AsyncWriteExt::write_all(self, buf).await?;
            AsyncWriteExt::flush(self).await
        }
    }
    fn shutdown<'a>(
        &'a mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move { AsyncWriteExt::shutdown(self).await }
    }
}

impl<S> ClientStream for HttpUpgradeStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "HttpUpgrade stream does not expose local_addr",
        ))
    }
}

// 鈹€鈹€ helpers 鈹€鈹€

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
}

fn parse_request_path(buf: &[u8]) -> Option<String> {
    let head = std::str::from_utf8(buf).ok()?;
    let first_line = head.lines().next()?;
    let parts: Vec<_> = first_line.split_whitespace().collect();
    if parts.len() >= 2 && parts[0] == "GET" {
        Some(parts[1].to_string())
    } else {
        None
    }
}

fn parse_status(buf: &[u8]) -> Option<u16> {
    let head = std::str::from_utf8(buf).ok()?;
    let first_line = head.lines().next()?;
    let parts: Vec<_> = first_line.split_whitespace().collect();
    if parts.len() >= 2 {
        parts[1].parse().ok()
    } else {
        None
    }
}

fn write_http_request(buf: &mut Vec<u8>, req: &Request<()>) {
    let path = req.uri().path_and_query().map_or("/", |u| u.as_str());
    let mut s = String::with_capacity(256);
    s.push_str(&format!("{} {} HTTP/1.1\r\n", req.method().as_str(), path));
    for (name, value) in req.headers() {
        s.push_str(&format!(
            "{}: {}\r\n",
            name.as_str(),
            value.to_str().unwrap_or("")
        ));
    }
    s.push_str("\r\n");
    buf.extend_from_slice(s.as_bytes());
}
