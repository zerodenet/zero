//! Async wrapper around Tls13Connection for tokio integration.
//!
//! Bridges sync Tls13Connection I/O with tokio's async TcpStream.

use std::io::{self, Read};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::TcpStream;

use super::handshake::{Tls13Config, Tls13Connection};

/// An async TLS 1.3 stream using our custom ClientHello handshake.
///
/// Implements `AsyncRead + AsyncWrite` as a drop-in replacement
/// for `tokio_rustls::client::TlsStream<TcpStream>`.
pub struct Tls13Stream {
    inner: TcpStream,
    conn: Tls13Connection,
}

impl Tls13Stream {
    /// Perform the full TLS 1.3 handshake asynchronously.
    pub async fn connect(stream: TcpStream, config: Tls13Config) -> io::Result<Self> {
        let mut conn = Tls13Connection::new(config)?;

        // Run handshake synchronously on the blocking thread pool
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

                if !conn.is_handshaking() {
                    break;
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

    pub fn get_ref(&self) -> &TcpStream {
        &self.inner
    }
}

impl AsyncRead for Tls13Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Return buffered plaintext first
        if let Some(plaintext) = self.conn.take_plaintext() {
            let n = plaintext.len().min(buf.remaining());
            buf.put_slice(&plaintext[..n]);
            return Poll::Ready(Ok(()));
        }

        // Read raw TLS data from underlying stream
        let mut raw = [0u8; 8192];
        let mut raw_buf = ReadBuf::new(&mut raw);
        match Pin::new(&mut self.inner).poll_read(cx, &mut raw_buf) {
            Poll::Ready(Ok(())) => {
                let filled = raw_buf.filled();
                if filled.is_empty() {
                    return Poll::Ready(Ok(()));
                }
                if let Err(e) = self.conn.read_tls(&mut io::Cursor::new(filled)) {
                    return Poll::Ready(Err(e));
                }
            }
            Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
            Poll::Pending => return Poll::Pending,
        }

        // Process new packets
        if let Err(e) = self.conn.process_new_packets() {
            return Poll::Ready(Err(e));
        }

        // Return plaintext
        if let Some(plaintext) = self.conn.take_plaintext() {
            let n = plaintext.len().min(buf.remaining());
            buf.put_slice(&plaintext[..n]);
        }
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for Tls13Stream {
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
