//! XHTTP transport (formerly SplitHTTP) — `split_http.rs`
//!
//! Splits a bidirectional stream across HTTP request(s) paired by an
//! `X-Session-Id` header. XTLS renamed SplitHTTP → XHTTP; the standalone
//! `quic` transport was removed in favour of XHTTP `stream-one` over H3.
//!
//! ## Modes (`SplitHttpConfig.mode`)
//! - **stream-one** (default, also selected by `auto`): a single
//!   bidirectional connection — a chunked POST body carries upload and a
//!   chunked response body carries download, both over the same TCP/TLS
//!   socket. This is the only mode that works as a **relay-chain final hop**,
//!   where the relay prefix provides a single stream. `XhttpStreamOne`
//!   implements it.
//! - **packet-up** / **stream-up**: the legacy two-connection model — a POST
//!   connection uploads, a separate GET connection downloads, paired by the
//!   server-side `SplitHttpRegistry`. Single-hop direct only; cannot be a
//!   relay final hop.
//!
//! ## Architecture
//! - Client `stream-one`: `connect_xhttp_stream_one` — one connection.
//! - Client two-connection: `connect_split_http` — POST + GET on two sockets.
//! - Server two-connection: `accept_split_http` pairs POST/GET by session ID.

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

// ── chunked transfer-encoding decoder (shared) ──

/// HTTP chunked-transfer-encoding decoder state machine.
///
/// Pure state over an internal byte buffer — it performs no I/O. The owner
/// feeds raw bytes via [`ChunkedDecoder::feed`] and drains decoded body bytes
/// via [`ChunkedDecoder::try_decode`]. This correctly handles arbitrary TCP
/// segmentation and consumes the trailing `\r\n` after each chunk's data
/// (the bug the original two-connection decoder had: it parsed the `\r\n`
/// terminator as a size line and broke on multi-chunk responses).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ChunkState {
    /// Reading the `<hex>[;ext]\r\n` chunk-size line.
    Size,
    /// Reading `chunk_remaining` bytes of chunk data.
    Data,
    /// Consuming the trailing `\r\n` after chunk data.
    Trailer,
}

/// Outcome of a single [`ChunkedDecoder::try_decode`] pass.
enum DecodeStep {
    /// Body bytes were produced, the stream hit EOF, or the output buffer is
    /// full — the caller returns `Poll::Ready(Ok(()))`.
    Done,
    /// More raw bytes are required — the caller feeds its source, then retries.
    NeedsMore,
}

struct ChunkedDecoder {
    /// Buffered raw bytes not yet consumed by the decoder.
    raw: Vec<u8>,
    /// Consumed offset within `raw`.
    raw_pos: usize,
    state: ChunkState,
    /// Bytes remaining in the current data chunk.
    chunk_remaining: usize,
    /// Set once the terminating `0` chunk has been seen.
    eof: bool,
}

impl ChunkedDecoder {
    fn new() -> Self {
        Self {
            raw: Vec::new(),
            raw_pos: 0,
            state: ChunkState::Size,
            chunk_remaining: 0,
            eof: false,
        }
    }

    /// Build a decoder pre-seeded with bytes already read past a header
    /// boundary (e.g. response body bytes captured during the handshake).
    fn with_prefetched(prefetched: Vec<u8>) -> Self {
        Self {
            raw: prefetched,
            raw_pos: 0,
            state: ChunkState::Size,
            chunk_remaining: 0,
            eof: false,
        }
    }

    fn feed(&mut self, bytes: &[u8]) {
        self.raw.extend_from_slice(bytes);
    }

    /// Drop consumed bytes from `raw` once fully drained.
    fn compact(&mut self) {
        if self.raw_pos >= self.raw.len() {
            self.raw.clear();
            self.raw_pos = 0;
        }
    }

    /// Try to decode body bytes into `buf`.
    ///
    /// Returns `Done` when output was produced, the stream hit EOF, or the
    /// output buffer is full; returns `NeedsMore` only when **nothing** was
    /// produced this pass and more raw bytes are required. This respects the
    /// `AsyncRead` contract — a caller must never return `Pending` while it
    /// has already filled the caller's buffer, otherwise the peer waits
    /// forever for an ack that never comes (a real deadlock the greedy
    /// version caused in the stream-one round-trip).
    fn try_decode(&mut self, buf: &mut ReadBuf<'_>) -> io::Result<DecodeStep> {
        if self.eof {
            return Ok(DecodeStep::Done);
        }
        let mut produced = false;
        loop {
            self.compact();
            match self.state {
                ChunkState::Size => {
                    let window = &self.raw[self.raw_pos..];
                    if let Some(rel) = window.windows(2).position(|w| w == b"\r\n") {
                        let line = &self.raw[self.raw_pos..self.raw_pos + rel];
                        self.raw_pos += rel + 2;
                        let hex_str = std::str::from_utf8(line).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                "split-http: non-UTF-8 chunk size",
                            )
                        })?;
                        // RFC 7230 chunk-size is hex digits, optionally followed
                        // by `;chunk-ext` — ignore any extension before parsing.
                        let hex_part = hex_str.split(';').next().unwrap_or("").trim();
                        let size = usize::from_str_radix(hex_part, 16).map_err(|_| {
                            io::Error::new(
                                io::ErrorKind::InvalidData,
                                format!("split-http: bad chunk-size: {hex_str}"),
                            )
                        })?;
                        if size == 0 {
                            self.eof = true;
                            return Ok(DecodeStep::Done);
                        }
                        self.chunk_remaining = size;
                        self.state = ChunkState::Data;
                        continue;
                    }
                    return Ok(if produced {
                        DecodeStep::Done
                    } else {
                        DecodeStep::NeedsMore
                    });
                }
                ChunkState::Data => {
                    let avail = self.raw.len() - self.raw_pos;
                    if avail == 0 {
                        return Ok(if produced {
                            DecodeStep::Done
                        } else {
                            DecodeStep::NeedsMore
                        });
                    }
                    if buf.remaining() == 0 {
                        return Ok(DecodeStep::Done);
                    }
                    let n = avail.min(self.chunk_remaining).min(buf.remaining());
                    buf.put_slice(&self.raw[self.raw_pos..self.raw_pos + n]);
                    self.raw_pos += n;
                    self.chunk_remaining -= n;
                    produced = true;
                    if self.chunk_remaining == 0 {
                        self.state = ChunkState::Trailer;
                    }
                    if buf.remaining() == 0 {
                        return Ok(DecodeStep::Done);
                    }
                    continue;
                }
                ChunkState::Trailer => {
                    let avail = self.raw.len() - self.raw_pos;
                    if avail < 2 {
                        return Ok(if produced {
                            DecodeStep::Done
                        } else {
                            DecodeStep::NeedsMore
                        });
                    }
                    // Consume the trailing `\r\n` after chunk data.
                    self.raw_pos += 2;
                    self.state = ChunkState::Size;
                    continue;
                }
            }
        }
    }
}

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
    /// Chunked-download decoder (shared with `XhttpStreamOne`).
    decoder: ChunkedDecoder,
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
            decoder: ChunkedDecoder::new(),
            write_finished: false,
        }
    }

    /// Construct with response-body bytes already read past the header
    /// boundary during the connect handshake.
    fn new_with_prefetched(reader: R, writer: W, prefetched: Vec<u8>) -> Self {
        Self {
            reader,
            writer,
            decoder: ChunkedDecoder::with_prefetched(prefetched),
            write_finished: false,
        }
    }
}

/// Accepted inbound XHTTP/SplitHTTP stream.
pub enum AcceptedSplitHttpInboundStream<S> {
    StreamOne(XhttpStreamOne<S>),
    Paired(SplitHttpStream<S>),
}

impl<S> AsyncRead for AcceptedSplitHttpInboundStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::StreamOne(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Paired(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl<S> AsyncWrite for AcceptedSplitHttpInboundStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::StreamOne(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Paired(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::StreamOne(stream) => Pin::new(stream).poll_flush(cx),
            Self::Paired(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::StreamOne(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Paired(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl<S> AsyncSocket for AcceptedSplitHttpInboundStream<S>
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

impl<S> ClientStream for AcceptedSplitHttpInboundStream<S> where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync
{
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

    // Preserve any response-body bytes read past the header boundary — they
    // are the start of the chunked download stream.
    let prefetched: Vec<u8> = if head_end < total {
        buf[head_end..total].to_vec()
    } else {
        Vec::new()
    };

    // reader = GET (download from server), writer = POST (upload to server)
    Ok(SplitHttpPairedStream::new_with_prefetched(
        get, post, prefetched,
    ))
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

/// Accept either XHTTP stream-one or paired SplitHTTP inbound transport.
pub async fn accept_xhttp_inbound<S>(
    stream: S,
    config: &SplitHttpConfig,
    registry: &SplitHttpRegistry,
) -> Result<Option<AcceptedSplitHttpInboundStream<S>>, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    if XhttpMode::parse(&config.mode).is_single_connection() {
        return accept_xhttp_stream_one(stream, config)
            .await
            .map(AcceptedSplitHttpInboundStream::StreamOne)
            .map(Some);
    }

    accept_split_http(stream, config, registry)
        .await
        .map(|stream| stream.map(AcceptedSplitHttpInboundStream::Paired))
}

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
        loop {
            match this.decoder.try_decode(buf)? {
                DecodeStep::Done => return Poll::Ready(Ok(())),
                DecodeStep::NeedsMore => {
                    let mut tmp = [0u8; 8192];
                    let mut rb = ReadBuf::new(&mut tmp);
                    match Pin::new(&mut this.reader).poll_read(cx, &mut rb) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Ready(Ok(())) => {
                            let filled = rb.filled();
                            if filled.is_empty() {
                                // Source EOF — treat as stream EOF.
                                return Poll::Ready(Ok(()));
                            }
                            this.decoder.feed(filled);
                        }
                    }
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

// ── XHTTP mode ──

/// Parsed XHTTP framing mode.
///
/// `Auto` and `StreamOne` both resolve to the single-connection path; the
/// difference is purely documentary. `PacketUp` / `StreamUp` select the
/// legacy two-connection model and are rejected on a single-stream relay
/// final hop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XhttpMode {
    Auto,
    PacketUp,
    StreamUp,
    StreamOne,
}

impl XhttpMode {
    /// Parse a mode string, treating the empty string as the default `auto`.
    /// Returns `None` for unknown values (validation rejects them earlier;
    /// this is a defensive fallback).
    pub fn parse(s: &str) -> Self {
        match s {
            "" | "auto" => XhttpMode::Auto,
            "packet-up" => XhttpMode::PacketUp,
            "stream-up" => XhttpMode::StreamUp,
            "stream-one" => XhttpMode::StreamOne,
            _ => XhttpMode::Auto,
        }
    }

    /// Whether the mode runs over a single bidirectional connection.
    ///
    /// `auto` resolves to `stream-one` on the client side: it is the only
    /// mode usable as a relay-chain final hop and the most compatible choice
    /// for asymmetric server deployments.
    pub fn is_single_connection(self) -> bool {
        matches!(self, XhttpMode::Auto | XhttpMode::StreamOne)
    }
}

// ── XhttpStreamOne (stream-one: single bidirectional connection) ──

/// Single-connection bidirectional XHTTP stream (`stream-one` mode).
///
/// Both upload and download flow over the same underlying connection:
/// - **upload** (`AsyncWrite`): HTTP/1.1 chunked-encoded POST body.
/// - **download** (`AsyncRead`): chunked-encoded response body.
///
/// This is the only XHTTP mode usable as a relay-chain final hop, where the
/// relay prefix delivers a single stream. The chunked download decoder is the
/// shared [`ChunkedDecoder`], which handles arbitrary TCP segmentation
/// (including the trailing `\r\n` after each chunk's data).
pub struct XhttpStreamOne<S> {
    inner: S,
    /// Chunked-download decoder (shared with `SplitHttpPairedStream`).
    decoder: ChunkedDecoder,
    // ── upload (chunked request encoder) ──
    /// True after the terminating `0\r\n\r\n` has been sent.
    write_finished: bool,
}

/// Connect via XHTTP `stream-one` over a single bidirectional connection.
///
/// Sends a chunked POST request (upload) and reads the `200` chunked response
/// (download) on the **same** connection. The returned stream reads the
/// chunked response body and writes the chunked request body concurrently —
/// safe because the underlying connection is full-duplex.
///
/// Pass either a raw TCP socket or an already-wrapped TLS stream. For a
/// relay-chain final hop, pass the relay carrier stream directly.
pub async fn connect_xhttp_stream_one<S>(
    stream: S,
    config: &SplitHttpConfig,
) -> Result<XhttpStreamOne<S>, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let host = config.host.as_deref().unwrap_or("localhost");
    let path = config.path.as_str();
    let session_id = gen_session_id();

    let mut s = stream;
    // Upload is a chunked POST body; download arrives as the chunked response
    // body on the same connection (stream-one).
    let req = format!(
        "POST {path} HTTP/1.1\r\n\
         Host: {host}\r\n\
         X-Session-Id: {session_id}\r\n\
         Transfer-Encoding: chunked\r\n\
         Content-Type: application/octet-stream\r\n\
         \r\n"
    );
    s.write_all(req.as_bytes()).await.map_err(EngineError::Io)?;
    s.flush().await.map_err(EngineError::Io)?;

    // Read response status line + headers. The server emits these immediately
    // in stream-one; the body streams afterwards on the same connection.
    let mut buf = vec![0u8; 8192];
    let mut total = 0;
    let head_end = loop {
        let n = s.read(&mut buf[total..]).await.map_err(EngineError::Io)?;
        if n == 0 {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "xhttp stream-one: unexpected EOF reading response",
            )));
        }
        total += n;
        if let Some(end) = find_header_end(&buf[..total]) {
            break end;
        }
        if total >= buf.len() {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "xhttp stream-one: response headers too large",
            )));
        }
    };

    let status = parse_status(&buf[..head_end]);
    if status != Some(200) {
        return Err(EngineError::Io(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("xhttp stream-one: expected 200, got {status:?}"),
        )));
    }

    // Preserve any response-body bytes read past the header boundary — they
    // are the start of the chunked download stream.
    let prefetched: Vec<u8> = if head_end < total {
        buf[head_end..total].to_vec()
    } else {
        Vec::new()
    };

    Ok(XhttpStreamOne {
        inner: s,
        decoder: ChunkedDecoder::with_prefetched(prefetched),
        write_finished: false,
    })
}

/// Accept an XHTTP `stream-one` connection on the server (inbound) side.
///
/// The mirror of [`connect_xhttp_stream_one`]: reads the client's chunked POST
/// request headers, immediately writes back a `200` chunked response on the
/// **same** connection, then returns a bidirectional stream. The returned
/// stream's `AsyncRead` decodes the client's upload (POST body) and its
/// `AsyncWrite` encodes the download (response body) — full-duplex over the
/// single connection.
///
/// The server emits the response headers before reading any upload body, so
/// there is no request/response deadlock when the client waits for the
/// response before uploading.
pub async fn accept_xhttp_stream_one<S>(
    stream: S,
    config: &SplitHttpConfig,
) -> Result<XhttpStreamOne<S>, EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut s = stream;

    // Read the client's POST request line + headers.
    let mut buf = vec![0u8; 8192];
    let mut total = 0;
    let head_end = loop {
        let n = s.read(&mut buf[total..]).await.map_err(EngineError::Io)?;
        if n == 0 {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "xhttp stream-one accept: unexpected EOF before request headers",
            )));
        }
        total += n;
        if let Some(end) = find_header_end(&buf[..total]) {
            break end;
        }
        if total >= buf.len() {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "xhttp stream-one accept: request headers too large",
            )));
        }
    };

    // Validate the request targets the configured path.
    validate_path(&buf[..head_end], config.path.as_str())?;

    // Respond 200 with a chunked body on the same connection. Sent before any
    // upload is consumed, so the client's connect handshake unblocks.
    s.write_all(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n")
        .await
        .map_err(EngineError::Io)?;
    s.flush().await.map_err(EngineError::Io)?;

    // Any request-body bytes already read past the header boundary are the
    // start of the client's upload — preserve them for the decoder.
    let prefetched: Vec<u8> = if head_end < total {
        buf[head_end..total].to_vec()
    } else {
        Vec::new()
    };

    Ok(XhttpStreamOne {
        inner: s,
        decoder: ChunkedDecoder::with_prefetched(prefetched),
        write_finished: false,
    })
}

// ── AsyncRead (chunked download decoder) ──

impl<S> AsyncRead for XhttpStreamOne<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        loop {
            match this.decoder.try_decode(buf)? {
                DecodeStep::Done => return Poll::Ready(Ok(())),
                DecodeStep::NeedsMore => {
                    let mut tmp = [0u8; 8192];
                    let mut rb = ReadBuf::new(&mut tmp);
                    match Pin::new(&mut this.inner).poll_read(cx, &mut rb) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Ready(Ok(())) => {
                            let filled = rb.filled();
                            if filled.is_empty() {
                                // Source EOF — treat as stream EOF.
                                return Poll::Ready(Ok(()));
                            }
                            this.decoder.feed(filled);
                        }
                    }
                }
            }
        }
    }
}

// ── AsyncWrite (chunked upload encoder) ──

impl<S> AsyncWrite for XhttpStreamOne<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
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

        let header = format!("{:x}\r\n", buf.len());
        let frame: Vec<u8> = header
            .as_bytes()
            .iter()
            .chain(buf.iter())
            .chain(b"\r\n".iter())
            .copied()
            .collect();

        match Pin::new(&mut this.inner).poll_write(cx, &frame) {
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

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.get_mut();
        if this.write_finished {
            return Poll::Ready(Ok(()));
        }
        this.write_finished = true;
        let write_res = Pin::new(&mut this.inner).poll_write(cx, b"0\r\n\r\n");
        match write_res {
            Poll::Ready(Ok(_)) => {
                let _ = Pin::new(&mut this.inner).poll_flush(cx);
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

// ── AsyncSocket ──

impl<S> AsyncSocket for XhttpStreamOne<S>
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

// ── ClientStream ──

impl<S> ClientStream for XhttpStreamOne<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn local_addr(&self) -> io::Result<SocketAddr> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "XHTTP stream-one stream does not expose local_addr",
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
