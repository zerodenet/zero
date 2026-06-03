//! Mieru TCP outbound — encrypted relay stream.
//!
//! Wraps a raw TCP stream with Mieru protocol encryption/decryption,
//! providing an `AsyncRead + AsyncWrite` interface for the proxy relay.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zero_protocol_mieru::MieruOutbound;

use crate::transport::TcpRelayStream;

/// A Mieru-encrypted TCP stream that transparently encrypts/decrypts
/// Mieru protocol segments during relay.
pub(crate) struct MieruTcpStream {
    inner: TcpRelayStream,
    outbound: MieruOutbound,
    write_buf: Vec<u8>,
    write_pos: usize,
    raw_read_buf: Vec<u8>,
    /// Buffered decrypted data from the last read.
    read_buf: Vec<u8>,
    read_pos: usize,
}

impl MieruTcpStream {
    pub fn new(inner: TcpRelayStream, outbound: MieruOutbound) -> Self {
        Self {
            inner,
            outbound,
            write_buf: Vec::new(),
            write_pos: 0,
            raw_read_buf: Vec::new(),
            read_buf: Vec::new(),
            read_pos: 0,
        }
    }

    /// Write plaintext → encrypt + send as Mieru data segment.
    fn poll_write_encrypted(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.write_buf.is_empty() {
            self.write_buf = self
                .outbound
                .encrypt_client_data(buf)
                .map_err(|e| io::Error::other(format!("mieru encrypt: {e}")))?;
            self.write_pos = 0;
        }

        while self.write_pos < self.write_buf.len() {
            match Pin::new(&mut self.inner).poll_write(cx, &self.write_buf[self.write_pos..]) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "mieru write zero",
                    )))
                }
                Poll::Ready(Ok(n)) => self.write_pos += n,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }

        self.write_buf.clear();
        self.write_pos = 0;
        Poll::Ready(Ok(buf.len()))
    }

    /// Read encrypted data → decrypt Mieru segment → return plaintext.
    fn poll_read_decrypted(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        // Serve buffered data first
        if self.read_pos < self.read_buf.len() {
            let remaining = &self.read_buf[self.read_pos..];
            let n = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..n]);
            self.read_pos += n;
            if self.read_pos >= self.read_buf.len() {
                self.read_buf.clear();
                self.read_pos = 0;
            }
            return Poll::Ready(Ok(()));
        }

        loop {
            match self
                .outbound
                .decrypt_server_data_with_consumed(&self.raw_read_buf)
            {
                Ok((segment, consumed)) => {
                    self.raw_read_buf.drain(..consumed);
                    let payload = segment.payload;
                    if payload.is_empty() {
                        continue;
                    }
                    let n = payload.len().min(buf.remaining());
                    buf.put_slice(&payload[..n]);
                    if n < payload.len() {
                        self.read_buf = payload[n..].to_vec();
                        self.read_pos = 0;
                    }
                    return Poll::Ready(Ok(()));
                }
                Err(error) if error == zero_core::Error::Protocol("mieru: need more data") => {
                    let mut scratch = [0u8; 4096];
                    let mut read_buf = ReadBuf::new(&mut scratch);
                    match Pin::new(&mut self.inner).poll_read(cx, &mut read_buf) {
                        Poll::Ready(Ok(())) => {
                            let filled = read_buf.filled();
                            if filled.is_empty() {
                                return Poll::Ready(Ok(()));
                            }
                            self.raw_read_buf.extend_from_slice(filled);
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                Err(e) => return Poll::Ready(Err(io::Error::other(format!("mieru decrypt: {e}")))),
            }
        }
    }
}

impl AsyncRead for MieruTcpStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::into_inner(self).poll_read_decrypted(cx, buf)
    }
}

impl AsyncWrite for MieruTcpStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::into_inner(self).poll_write_encrypted(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
