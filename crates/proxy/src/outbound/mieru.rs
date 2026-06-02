//! Mieru TCP outbound — encrypted relay stream.
//!
//! Wraps a raw TCP stream with Mieru protocol encryption/decryption,
//! providing an `AsyncRead + AsyncWrite` interface for the proxy relay.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zero_protocol_mieru::{DataMetadata, MieruOutbound, Segment, DATA_SERVER_TO_CLIENT};

use crate::transport::TcpRelayStream;

/// A Mieru-encrypted TCP stream that transparently encrypts/decrypts
/// Mieru protocol segments during relay.
pub(crate) struct MieruTcpStream {
    inner: TcpRelayStream,
    outbound: MieruOutbound,
    /// Buffered decrypted data from the last read.
    read_buf: Vec<u8>,
    read_pos: usize,
}

impl MieruTcpStream {
    pub fn new(inner: TcpRelayStream, outbound: MieruOutbound) -> Self {
        Self {
            inner,
            outbound,
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
        // Encrypt data as a Mieru data segment
        let segment = self
            .outbound
            .encrypt_client_data(buf)
            .map_err(|e| io::Error::other(format!("mieru encrypt: {e}")))?;

        // Write encrypted segment to underlying stream
        let mut written = 0;
        while written < segment.len() {
            match Pin::new(&mut self.inner).poll_write(cx, &segment[written..]) {
                Poll::Ready(Ok(n)) if n > 0 => written += n,
                Poll::Ready(Ok(_)) => break,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending if written > 0 => break,
                Poll::Pending => return Poll::Pending,
            }
        }

        // Report the plaintext bytes as written (caller doesn't know about framing)
        if written > 0 {
            Poll::Ready(Ok(buf.len()))
        } else {
            Poll::Pending
        }
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

        // Read a chunk from the underlying stream
        let mut raw = vec![0u8; 4096];
        let mut read_buf = ReadBuf::new(&mut raw);
        match Pin::new(&mut self.inner).poll_read(cx, &mut read_buf) {
            Poll::Ready(Ok(())) => {
                let filled = read_buf.filled().len();
                if filled == 0 {
                    return Poll::Ready(Ok(())); // EOF
                }
                raw.truncate(filled);

                // Decrypt the mieru segment
                match self.outbound.decrypt_server_data(&raw) {
                    Ok(segment) => {
                        let payload = segment.payload;
                        let n = payload.len().min(buf.remaining());
                        buf.put_slice(&payload[..n]);
                        // Buffer any remaining decrypted data
                        if n < payload.len() {
                            self.read_buf = payload[n..].to_vec();
                            self.read_pos = 0;
                        }
                        Poll::Ready(Ok(()))
                    }
                    Err(e) => Poll::Ready(Err(io::Error::other(format!("mieru decrypt: {e}")))),
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
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
