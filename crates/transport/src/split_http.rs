//! SplitHTTP (XHTTP) transport — split_http.rs
//!
//! Splits a bidirectional stream across HTTP POST (upload) + GET (download)
//! paired by an X-Session-Id header.
//!
//! ## Modes
//! - **stream-one** (v1): single TCP connection, HTTP pipelining (client)
//! - **multi-connection** (v2): POST+GET on separate connections, server-side
//!   `SplitHttpRegistry` pairs them by session ID
//!
//! ## Architecture
//! - Client: stream-one (POST+GET pipelined on one TCP socket)
//! - Server: `accept_split_http` uses a registry to pair separate POST/GET
//!   connections. The SECOND connection to arrive creates and returns the
//!   paired stream; the FIRST returns `None` (consumed by partner).

use std::any::Any;
use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Instant;

use std::net::SocketAddr;

use http::Request;
use rand::Rng;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::{oneshot, Mutex};
use zero_config::SplitHttpConfig;
use zero_engine::EngineError;
use zero_platform_tokio::ClientStream;
use zero_traits::AsyncSocket;

// ── Session Registry ──

/// Pending half of a SplitHTTP session.
///
/// Stored when one HTTP connection (POST or GET) arrives before its partner.
/// The second connection to arrive takes the first's stream and wakes it.
struct SplitHttpPending {
    stream: Box<dyn Any + Send>,
    #[allow(dead_code)]
    notify: oneshot::Sender<()>,
    #[allow(dead_code)]
    created: Instant,
}

/// Server-side registry pairing POST and GET connections by session ID.
///
/// Created once per listener and shared across all accept calls.
/// Uses type-erased (`Box<dyn Any>`) storage so the registry itself is not
/// generic; each `accept_split_http` call downcasts to the concrete `S`.
pub struct SplitHttpRegistry {
    inner: Arc<Mutex<HashMap<String, SplitHttpPending>>>,
}

impl SplitHttpRegistry {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Clone for SplitHttpRegistry {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl Default for SplitHttpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── SplitHttpPairedStream ──

/// Bidirectional stream combining POST body (read) and GET response (write).
///
/// - `AsyncRead`: reads from `reader`, decoding chunked transfer encoding
/// - `AsyncWrite`: writes to `writer`, encoding as chunked transfer encoding
///
/// In stream-one mode `R = W = S` (same TCP connection).
/// In multi-connection mode they differ (POST TCP and GET TCP).
pub struct SplitHttpPairedStream<R, W> {
    reader: R,
    writer: W,
    /// Remaining bytes in the current data chunk (0 = need new chunk-size).
    chunk_remaining: usize,
    /// Line buffer for chunk-size parsing.
    line_buf: Vec<u8>,
    line_pos: usize,
    /// True after terminating 0-chunk sent.
    write_finished: bool,
}

/// Convenience alias: same connection for both directions (stream-one mode).
pub type SplitHttpStream<S> = SplitHttpPairedStream<S, S>;

impl<R, W> SplitHttpPairedStream<R, W> {
    fn new(reader: R, writer: W) -> Self {
        Self {
            reader,
            writer,
            chunk_remaining: 0,
            line_buf: Vec::new(),
            line_pos: 0,
            write_finished: false,
        }
    }
}

// ── client (outbound) connect ──

/// Connect via SplitHTTP.
///
/// Takes two streams:
/// - `post_stream`: used for POST (upload, chunked body) → writer
/// - `get_stream`: used for GET (download, chunked response) → reader
///
/// Returns `SplitHttpPairedStream<SR (reader=GET), SW (writer=POST)>`.
///
/// In **stream-one** mode pass the same stream for both.
/// In **multi-connection** mode pass separate TCP connections.
pub async fn connect_split_http<SR>(
    post_stream: SR,
    get_stream: SR,
    config: &SplitHttpConfig,
) -> Result<SplitHttpStream<SR>, EngineError>
where
    SR: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let host = config.host.as_deref().unwrap_or("localhost");
    let path = config.path.as_str();
    let session_id = gen_session_id();

    let post_req = Request::builder()
        .method("POST")
        .uri(path)
        .header("Host", host)
        .header("X-Session-Id", &session_id)
        .header("Transfer-Encoding", "chunked")
        .header("Content-Type", "application/octet-stream")
        .body(())
        .map_err(|e| EngineError::Io(io::Error::other(format!("split-http post request: {e}"))))?;

    let get_req = Request::builder()
        .method("GET")
        .uri(path)
        .header("Host", host)
        .header("X-Session-Id", &session_id)
        .body(())
        .map_err(|e| EngineError::Io(io::Error::other(format!("split-http get request: {e}"))))?;

    // Write POST headers to post_stream
    let mut post = post_stream;
    let mut req_bytes = Vec::new();
    write_http_request(&mut req_bytes, &post_req);
    post.write_all(&req_bytes).await.map_err(EngineError::Io)?;

    // Write GET headers to get_stream and read response
    let mut get = get_stream;
    req_bytes.clear();
    write_http_request(&mut req_bytes, &get_req);
    get.write_all(&req_bytes).await.map_err(EngineError::Io)?;

    let mut buf = vec![0u8; 4096];
    let mut total = 0;
    let head_end = loop {
        let n = get.read(&mut buf[total..]).await.map_err(EngineError::Io)?;
        if n == 0 {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "split-http connect: unexpected EOF reading GET response",
            )));
        }
        total += n;
        if let Some(end) = find_header_end(&buf[..total]) {
            break end;
        }
        if total >= buf.len() {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "split-http connect: response headers too large",
            )));
        }
    };

    let status = parse_status(&buf[..head_end]);
    if status != Some(200) {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("split-http connect: expected 200, got {status:?}"),
        )));
    }

    // reader = GET (download from server), writer = POST (upload to server)
    Ok(SplitHttpPairedStream::new(get, post))
}

// ── server (inbound) accept ──

/// Result of accepting a SplitHTTP connection.
///
/// - `Some(stream)` — paired stream ready for relay (second connection won)
/// - `None` — this connection was paired by the other side (first connection,
///   call site should drop the task silently)
pub async fn accept_split_http<S>(
    stream: S,
    config: &SplitHttpConfig,
    registry: &SplitHttpRegistry,
) -> Result<Option<SplitHttpStream<S>>, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut io = stream;

    // ── Read HTTP headers ──
    let mut buf = vec![0u8; 4096];
    let mut total = 0;
    let head_end = loop {
        let n = io.read(&mut buf[total..]).await.map_err(EngineError::Io)?;
        if n == 0 {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "split-http accept: unexpected EOF",
            )));
        }
        total += n;
        if let Some(end) = find_header_end(&buf[..total]) {
            break end;
        }
        if total >= buf.len() {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "split-http accept: headers too large",
            )));
        }
    };

    let (method, session_id) = parse_method_and_session(&buf[..head_end])?;
    validate_path(&buf[..head_end], config.path.as_str())?;

    let _expected = config.path.as_str();

    match method.as_str() {
        "POST" => {
            let mut reg = registry.inner.lock().await;
            if let Some(pending) = reg.remove(&session_id) {
                drop(reg);
                // GET was waiting — we're the second connection, we win
                let mut get_stream = *pending.stream.downcast::<S>().map_err(|_| {
                    EngineError::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "split-http: type mismatch",
                    ))
                })?;
                write_get_response(&mut get_stream).await?;
                Ok(Some(SplitHttpPairedStream::new(io, get_stream)))
            } else {
                let (notify_tx, notify_rx) = oneshot::channel();
                reg.insert(
                    session_id.clone(),
                    SplitHttpPending {
                        stream: Box::new(io),
                        notify: notify_tx,
                        created: Instant::now(),
                    },
                );
                drop(reg);

                match tokio::time::timeout(std::time::Duration::from_secs(60), notify_rx).await {
                    Ok(Ok(())) => Ok(None),
                    Ok(Err(_)) => Ok(None),
                    Err(_) => {
                        registry.inner.lock().await.remove(&session_id);
                        Err(EngineError::Io(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "split-http: POST timed out waiting for GET",
                        )))
                    }
                }
            }
        }
        "GET" => {
            let mut reg = registry.inner.lock().await;
            if let Some(pending) = reg.remove(&session_id) {
                drop(reg);
                // POST was waiting — we're the second, we win
                write_get_response(&mut io).await?;
                let post_stream = *pending.stream.downcast::<S>().map_err(|_| {
                    EngineError::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "split-http: type mismatch",
                    ))
                })?;
                // reader = post_stream (upload), writer = io (download)
                Ok(Some(SplitHttpPairedStream::new(post_stream, io)))
            } else {
                let (notify_tx, notify_rx) = oneshot::channel();
                reg.insert(
                    session_id.clone(),
                    SplitHttpPending {
                        stream: Box::new(io),
                        notify: notify_tx,
                        created: Instant::now(),
                    },
                );
                drop(reg);

                match tokio::time::timeout(std::time::Duration::from_secs(60), notify_rx).await {
                    Ok(Ok(())) => Ok(None),
                    Ok(Err(_)) => Ok(None),
                    Err(_) => {
                        registry.inner.lock().await.remove(&session_id);
                        Err(EngineError::Io(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "split-http: GET timed out waiting for POST",
                        )))
                    }
                }
            }
        }
        other => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("split-http: unexpected method {other}"),
        ))),
    }
}

// ── AsyncRead (chunked decoder from reader) ──

impl<R, W> AsyncRead for SplitHttpPairedStream<R, W>
where
    R: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    W: Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();

        // Ensure line buffer has capacity
        if this.line_buf.capacity() < 1024 {
            this.line_buf.resize(1024, 0);
        }

        loop {
            // Serve current chunk data: read directly into the user's buffer,
            // bounded by remaining chunk size.
            if this.chunk_remaining > 0 {
                let limit = this.chunk_remaining.min(buf.remaining());
                let mut restricted = buf.take(limit);
                let inner = unsafe { Pin::new_unchecked(&mut this.reader) };
                match inner.poll_read(cx, &mut restricted) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Ready(Ok(())) => {
                        let added = restricted.filled().len();
                        if added == 0 {
                            this.chunk_remaining = 0;
                        } else {
                            this.chunk_remaining -= added;
                        }
                        // restricted dropped → original buf auto-updated
                        return Poll::Ready(Ok(()));
                    }
                }
            }

            // Parse next chunk-size line from reader
            let inner = unsafe { Pin::new_unchecked(&mut this.reader) };
            let fill_start = this.line_pos;
            if fill_start >= this.line_buf.len() {
                this.line_buf.resize(this.line_buf.len() + 256, 0);
            }
            let mut read_buf = ReadBuf::new(&mut this.line_buf[fill_start..]);
            match inner.poll_read(cx, &mut read_buf) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Ready(Ok(())) => {
                    let filled = read_buf.filled().len();
                    if filled == 0 {
                        return Poll::Ready(Ok(())); // EOF
                    }
                    this.line_pos += filled;

                    // Scan for \r\n
                    if let Some(line_end) = this.line_buf[..this.line_pos]
                        .windows(2)
                        .position(|w| w == b"\r\n")
                    {
                        let hex_str =
                            std::str::from_utf8(&this.line_buf[..line_end]).map_err(|_| {
                                io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "split-http: invalid chunk-size hex",
                                )
                            })?;
                        let hex_str = hex_str.trim();
                        let size = usize::from_str_radix(hex_str, 16).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("split-http: bad chunk-size: {hex_str}"),
                            )
                        })?;

                        // Reset line buffer
                        this.line_pos = 0;

                        if size == 0 {
                            // Terminating chunk
                            return Poll::Ready(Ok(())); // EOF: 0 bytes filled
                        }

                        this.chunk_remaining = size;
                        continue;
                    }

                    // Line continues — need more data
                    continue;
                }
            }
        }
    }
}

// ── AsyncWrite (chunked encoder to writer) ──

impl<R, W> AsyncWrite for SplitHttpPairedStream<R, W>
where
    W: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    R: Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        if buf.is_empty() || self.write_finished {
            return Poll::Ready(Ok(0));
        }

        let this = self.get_mut();
        let inner = unsafe { Pin::new_unchecked(&mut this.writer) };

        let header = format!("{:x}\r\n", buf.len());
        let frame: Vec<u8> = header
            .as_bytes()
            .iter()
            .chain(buf.iter())
            .chain(b"\r\n".iter())
            .copied()
            .collect();

        match inner.poll_write(cx, &frame) {
            Poll::Ready(Ok(written)) => {
                let data_written = if written >= header.len() + 2 {
                    buf.len().min(written - header.len() - 2)
                } else {
                    0
                };
                Poll::Ready(Ok(data_written))
            }
            other => other,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let inner = unsafe { Pin::new_unchecked(&mut self.writer) };
        inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.get_mut();
        if this.write_finished {
            return Poll::Ready(Ok(()));
        }
        this.write_finished = true;
        let write_res = {
            let inner = unsafe { Pin::new_unchecked(&mut this.writer) };
            inner.poll_write(cx, b"0\r\n\r\n")
        };
        match write_res {
            Poll::Ready(Ok(_)) => {
                let inner = unsafe { Pin::new_unchecked(&mut this.writer) };
                let _ = inner.poll_flush(cx);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

// ── AsyncSocket ──

impl<R, W> AsyncSocket for SplitHttpPairedStream<R, W>
where
    R: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    W: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        AsyncReadExt::read(self, buf).await
    }
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        AsyncWriteExt::write_all(self, buf).await?;
        AsyncWriteExt::flush(self).await
    }
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        AsyncWriteExt::shutdown(self).await
    }
}

// ── ClientStream ──

impl<R, W> ClientStream for SplitHttpPairedStream<R, W>
where
    R: AsyncRead + AsyncWrite + Unpin + Send + Sync,
    W: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "SplitHttp stream does not expose local_addr",
        ))
    }
}

// ── helpers ──

fn gen_session_id() -> String {
    let id: u64 = rand::rng().random();
    format!("{id:016x}")
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4)
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

/// Parse HTTP method and X-Session-Id header value from raw headers.
fn parse_method_and_session(buf: &[u8]) -> Result<(String, String), EngineError> {
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

    // Extract X-Session-Id header
    let session_id = head
        .lines()
        .find_map(|line| {
            let lower = line.to_lowercase();
            lower
                .strip_prefix("x-session-id:")
                .map(|val| val.trim().to_string())
        })
        .unwrap_or_else(|| "0".to_string());

    Ok((method, session_id))
}

fn validate_path(buf: &[u8], expected: &str) -> Result<(), EngineError> {
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

async fn write_get_response<W: AsyncWrite + Unpin>(writer: &mut W) -> Result<(), EngineError> {
    let resp = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n";
    writer
        .write_all(resp.as_bytes())
        .await
        .map_err(EngineError::Io)
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
