use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::VLESS_VERSION;

enum VlessResponseState {
    Version,
    AddonLength,
    Addon { remaining: usize },
    Ready,
}

pub struct DeferredVlessResponseStream<S> {
    inner: S,
    state: VlessResponseState,
}

impl<S> DeferredVlessResponseStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            state: VlessResponseState::Version,
        }
    }
}

impl<S> AsyncRead for DeferredVlessResponseStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            match self.state {
                VlessResponseState::Version => {
                    let version = match poll_read_one(&mut self.inner, cx)? {
                        Poll::Ready(version) => version,
                        Poll::Pending => return Poll::Pending,
                    };
                    if version != VLESS_VERSION {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "unsupported VLESS response version",
                        )));
                    }
                    self.state = VlessResponseState::AddonLength;
                }
                VlessResponseState::AddonLength => {
                    let length = match poll_read_one(&mut self.inner, cx)? {
                        Poll::Ready(length) => length,
                        Poll::Pending => return Poll::Pending,
                    };
                    self.state = if length == 0 {
                        VlessResponseState::Ready
                    } else {
                        VlessResponseState::Addon {
                            remaining: length as usize,
                        }
                    };
                }
                VlessResponseState::Addon { remaining } => {
                    let consumed = match poll_discard(&mut self.inner, cx, remaining)? {
                        Poll::Ready(consumed) => consumed,
                        Poll::Pending => return Poll::Pending,
                    };
                    let remaining = remaining.saturating_sub(consumed);
                    self.state = if remaining == 0 {
                        VlessResponseState::Ready
                    } else {
                        VlessResponseState::Addon { remaining }
                    };
                }
                VlessResponseState::Ready => {
                    return Pin::new(&mut self.inner).poll_read(cx, buf);
                }
            }
        }
    }
}

impl<S> AsyncWrite for DeferredVlessResponseStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

fn poll_read_one<S>(inner: &mut S, cx: &mut Context<'_>) -> io::Result<Poll<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut byte = [0_u8; 1];
    let mut read_buf = ReadBuf::new(&mut byte);
    match Pin::new(inner).poll_read(cx, &mut read_buf) {
        Poll::Ready(Ok(())) if read_buf.filled().is_empty() => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "EOF while reading VLESS response",
        )),
        Poll::Ready(Ok(())) => Ok(Poll::Ready(byte[0])),
        Poll::Ready(Err(error)) => Err(error),
        Poll::Pending => Ok(Poll::Pending),
    }
}

fn poll_discard<S>(inner: &mut S, cx: &mut Context<'_>, remaining: usize) -> io::Result<Poll<usize>>
where
    S: AsyncRead + Unpin,
{
    let mut discard = [0_u8; 256];
    let len = remaining.min(discard.len());
    let mut read_buf = ReadBuf::new(&mut discard[..len]);
    match Pin::new(inner).poll_read(cx, &mut read_buf) {
        Poll::Ready(Ok(())) if read_buf.filled().is_empty() => Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "EOF while reading VLESS response addon",
        )),
        Poll::Ready(Ok(())) => Ok(Poll::Ready(read_buf.filled().len())),
        Poll::Ready(Err(error)) => Err(error),
        Poll::Pending => Ok(Poll::Pending),
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    async fn deferred_vless_response_discards_header_before_payload() {
        let (client, mut server) = tokio::io::duplex(64);
        let mut stream = DeferredVlessResponseStream::new(client);

        server
            .write_all(&[
                VLESS_VERSION,
                0x03,
                b'a',
                b'b',
                b'c',
                b'p',
                b'o',
                b'n',
                b'g',
            ])
            .await
            .expect("write response");

        let mut payload = [0_u8; 4];
        stream.read_exact(&mut payload).await.expect("read payload");

        assert_eq!(&payload, b"pong");
    }

    #[tokio::test]
    async fn deferred_vless_response_forwards_writes_before_response_arrives() {
        let (client, mut server) = tokio::io::duplex(64);
        let mut stream = DeferredVlessResponseStream::new(client);

        stream.write_all(b"ping").await.expect("write request");

        let mut request = [0_u8; 4];
        server.read_exact(&mut request).await.expect("read request");

        assert_eq!(&request, b"ping");
    }
}
