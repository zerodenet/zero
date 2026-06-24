use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use mieru::{MieruAccept, MieruInboundDataCodec};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::transport::TcpRelayStream;

/// Wraps a `TcpRelayStream` carrying the Mieru session cipher state
/// for the server-to-client (download) direction.
pub(crate) struct MieruClientStream {
    inner: TcpRelayStream,
    codec: MieruInboundDataCodec,
    /// Buffered decrypted data from a partial segment read.
    read_buf: Vec<u8>,
    read_pos: usize,
    raw_read_buf: Vec<u8>,
    write_buf: Vec<u8>,
    write_pos: usize,
    write_plain_len: usize,
}

impl MieruClientStream {
    pub(crate) fn new(inner: TcpRelayStream, accept: MieruAccept) -> Self {
        let (codec, read_buf) = MieruInboundDataCodec::new(accept);
        Self {
            inner,
            codec,
            read_buf,
            read_pos: 0,
            raw_read_buf: Vec::new(),
            write_buf: Vec::new(),
            write_pos: 0,
            write_plain_len: 0,
        }
    }
}

impl AsyncRead for MieruClientStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = Pin::into_inner(self);

        if this.read_pos < this.read_buf.len() {
            let remaining = &this.read_buf[this.read_pos..];
            let n = remaining.len().min(buf.remaining());
            buf.put_slice(&remaining[..n]);
            this.read_pos += n;
            if this.read_pos >= this.read_buf.len() {
                this.read_buf.clear();
                this.read_pos = 0;
            }
            return Poll::Ready(Ok(()));
        }

        loop {
            match this
                .codec
                .decrypt_client_data_with_consumed(&this.raw_read_buf)
            {
                Ok((segment, consumed)) => {
                    this.raw_read_buf.drain(..consumed);

                    let payload = segment.payload;
                    if payload.is_empty() {
                        continue;
                    }

                    let n = payload.len().min(buf.remaining());
                    buf.put_slice(&payload[..n]);
                    if n < payload.len() {
                        this.read_buf = payload[n..].to_vec();
                        this.read_pos = 0;
                    }
                    return Poll::Ready(Ok(()));
                }
                Err(zero_core::Error::Protocol("mieru: need more data")) => {}
                Err(error) => {
                    return Poll::Ready(Err(io::Error::other(format!("mieru decrypt: {error}"))));
                }
            }

            let before = this.raw_read_buf.len();
            this.raw_read_buf.resize(before + 8192, 0);
            let mut read_buf = ReadBuf::new(&mut this.raw_read_buf[before..]);
            match Pin::new(&mut this.inner).poll_read(cx, &mut read_buf) {
                Poll::Ready(Ok(())) => {
                    let filled = read_buf.filled().len();
                    this.raw_read_buf.truncate(before + filled);
                    if filled == 0 {
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Err(e)) => {
                    this.raw_read_buf.truncate(before);
                    return Poll::Ready(Err(e));
                }
                Poll::Pending => {
                    this.raw_read_buf.truncate(before);
                    return Poll::Pending;
                }
            }
        }
    }
}

impl AsyncWrite for MieruClientStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = Pin::into_inner(self);

        if this.write_buf.is_empty() {
            match this.codec.encrypt_server_data(buf) {
                Ok(segment) => {
                    this.write_buf = segment;
                    this.write_pos = 0;
                    this.write_plain_len = buf.len();
                }
                Err(_) => return Poll::Ready(Err(io::Error::other("mieru encrypt failed"))),
            }
        }

        while this.write_pos < this.write_buf.len() {
            match Pin::new(&mut this.inner).poll_write(cx, &this.write_buf[this.write_pos..]) {
                Poll::Ready(Ok(0)) => {
                    return Poll::Ready(Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "mieru write zero",
                    )));
                }
                Poll::Ready(Ok(n)) => {
                    this.write_pos += n;
                }
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
        }

        let written = this.write_plain_len;
        this.write_buf.clear();
        this.write_pos = 0;
        this.write_plain_len = 0;
        Poll::Ready(Ok(written))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
