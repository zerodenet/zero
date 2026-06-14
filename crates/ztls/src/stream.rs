//! Async wrapper around Tls13Connection for tokio integration.
//!
//! Provides TLS 1.3 handshake over both concrete TcpStream (fast path with
//! spawn_blocking + into_std) and generic AsyncRead+AsyncWrite streams
//! (async handshake loop for relay-stream TLS).

use std::io::{self, Read};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;

use super::handshake::{Tls13Config, Tls13Connection};

/// An async TLS 1.3 stream using our custom ClientHello handshake.
///
/// Generic over the inner stream type `S`. Implements `AsyncRead + AsyncWrite`.
///
/// # Construction
///
/// - `Tls13Stream::connect(tcp_stream, config)` — fast path for fresh sockets
///   (uses `spawn_blocking` + `into_std()`).
/// - `Tls13Stream::connect_async(stream, config)` — generic async handshake for
///   any `AsyncRead + AsyncWrite + Unpin + Send + 'static` stream (relay streams,
///   trait objects, etc.).
pub struct Tls13Stream<S> {
    inner: S,
    conn: Tls13Connection,
}

/// Fast-path constructor: fresh TcpStream.
impl Tls13Stream<TcpStream> {
    /// Perform the full TLS 1.3 handshake over a concrete TcpStream.
    ///
    /// Uses `spawn_blocking` + `into_std()` for maximum throughput on
    /// fresh-socket connections.
    pub async fn connect(stream: TcpStream, config: Tls13Config) -> io::Result<Self> {
        let mut conn = Tls13Connection::new(config)?;

        let mut stream_std = stream.into_std()?;
        let (conn, stream_std) = tokio::task::spawn_blocking(move || {
            loop {
                if conn.wants_write() {
                    let mut buf = Vec::new();
                    conn.write_tls(&mut buf)?;
                    if !buf.is_empty() {
                        std::io::Write::write_all(&mut stream_std, &buf)?;
                    }
                }

                let _ = conn.process_new_packets()?;

                // The handshake only completes once our Finished flight has
                // actually been written; `is_handshaking()` flips to false
                // while the Finished is still queued, so also require no
                // pending output before breaking.
                if !conn.is_handshaking() && !conn.wants_write() {
                    break;
                }

                // If processing just queued output (our Finished), flush it
                // before trying to read — the peer may be waiting for it.
                if conn.wants_write() {
                    continue;
                }

                if conn.wants_read() {
                    let mut buf = [0u8; 8192];
                    match stream_std.read(&mut buf) {
                        Ok(0) => {
                            return Err(io::Error::new(
                                io::ErrorKind::UnexpectedEof,
                                "TLS handshake: connection closed",
                            ))
                        }
                        Ok(n) => {
                            conn.read_tls(&mut io::Cursor::new(&buf[..n]))?;
                        }
                        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                            continue;
                        }
                        Err(e) => return Err(e),
                    }
                }
            }
            Ok::<_, io::Error>((conn, stream_std))
        })
        .await
        .map_err(|e| io::Error::other(e))??;

        let stream = TcpStream::from_std(stream_std)?;

        Ok(Self {
            inner: stream,
            conn,
        })
    }
}

/// Generic constructor for any AsyncRead + AsyncWrite stream.
impl<S> Tls13Stream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    /// Perform the full TLS 1.3 handshake asynchronously over a generic stream.
    ///
    /// Uses async I/O primitives directly — no `spawn_blocking` or `into_std()`.
    /// Suitable for relay streams, trait objects, and any `AsyncRead + AsyncWrite`
    /// carrier that is not a concrete `TcpStream`.
    pub async fn connect_async(mut stream: S, config: Tls13Config) -> io::Result<Self> {
        let mut conn = Tls13Connection::new(config)?;

        loop {
            if conn.wants_write() {
                let mut buf = Vec::new();
                conn.write_tls(&mut buf)?;
                if !buf.is_empty() {
                    stream.write_all(&buf).await?;
                }
            }

            let _ = conn.process_new_packets()?;

            // The handshake only completes once our Finished flight has
            // actually been written; `is_handshaking()` flips to false while
            // the Finished is still queued, so also require no pending output
            // before breaking.
            if !conn.is_handshaking() && !conn.wants_write() {
                break;
            }

            // If processing just queued output (our Finished), flush it before
            // trying to read — the peer may be blocked waiting for it, so a
            // read here would deadlock.
            if conn.wants_write() {
                continue;
            }

            if conn.wants_read() {
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).await?;
                if n == 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "TLS handshake: connection closed",
                    ));
                }
                conn.read_tls(&mut io::Cursor::new(&buf[..n]))?;
            }
        }

        Ok(Self {
            inner: stream,
            conn,
        })
    }
}

impl<S> Tls13Stream<S> {
    pub fn get_ref(&self) -> &S {
        &self.inner
    }
}

impl<S> AsyncRead for Tls13Stream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Serve already-decrypted plaintext first.
        let n = self.conn.read_plaintext(buf.initialize_unfilled());
        if n > 0 {
            buf.advance(n);
            return Poll::Ready(Ok(()));
        }

        // Loop until we either produce plaintext, hit a genuine EOF, or the
        // underlying stream has no more data right now. Returning Ok with zero
        // bytes filled would be interpreted by tokio as EOF, so a read that
        // brings in a partial (or non-application) record must keep reading.
        loop {
            let mut raw = [0u8; 8192];
            let mut raw_buf = ReadBuf::new(&mut raw);
            match Pin::new(&mut self.inner).poll_read(cx, &mut raw_buf) {
                Poll::Ready(Ok(())) => {
                    let filled = raw_buf.filled();
                    if filled.is_empty() {
                        // Underlying stream closed. Surface any final plaintext
                        // already buffered above; otherwise this is a clean EOF.
                        return Poll::Ready(Ok(()));
                    }
                    if let Err(e) = self.conn.read_tls(&mut io::Cursor::new(filled)) {
                        return Poll::Ready(Err(e));
                    }
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }

            if let Err(e) = self.conn.process_new_packets() {
                return Poll::Ready(Err(e));
            }

            let n = self.conn.read_plaintext(buf.initialize_unfilled());
            if n > 0 {
                buf.advance(n);
                return Poll::Ready(Ok(()));
            }
            // Partial record or a non-application record (e.g. an alert) was
            // consumed; loop to read more.
        }
    }
}

impl<S> AsyncWrite for Tls13Stream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        self.conn.write_plaintext(buf);

        // Encrypt and flush to underlying stream
        let mut enc = Vec::with_capacity(OUTGOING_BUFFER_LIMIT);
        if let Err(e) = self.conn.write_tls(&mut enc) {
            return Poll::Ready(Err(e));
        }
        if !enc.is_empty() {
            let mut offset = 0;
            while offset < enc.len() {
                match Pin::new(&mut self.inner).poll_write(cx, &enc[offset..]) {
                    Poll::Ready(Ok(0)) => {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::WriteZero,
                            "TLS write zero",
                        )));
                    }
                    Poll::Ready(Ok(n)) => offset += n,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                }
            }
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

const OUTGOING_BUFFER_LIMIT: usize = 64 * 1024;
