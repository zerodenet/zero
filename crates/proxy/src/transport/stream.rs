use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

#[cfg(feature = "inbound-mixed")]
use std::cmp;
#[cfg(any(feature = "inbound-socks5", feature = "inbound-mixed"))]
use std::net::SocketAddr;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zero_platform_tokio::TokioSocket;
use zero_traits::AsyncSocket;

pub(crate) trait ClientStream:
    AsyncSocket<Error = io::Error> + AsyncRead + AsyncWrite + Send + Sync + Unpin
{
    #[cfg(feature = "inbound-socks5")]
    fn local_addr(&self) -> io::Result<SocketAddr>;
}

impl ClientStream for TokioSocket {
    #[cfg(feature = "inbound-socks5")]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.local_addr()
    }
}

pub(crate) struct TcpRelayStream {
    inner: Box<dyn RelayIo>,
}

trait RelayIo: AsyncRead + AsyncWrite + Send + Sync + Unpin {}

impl<T> RelayIo for T where T: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static {}

impl TcpRelayStream {
    pub(crate) fn new<S>(stream: S) -> Self
    where
        S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    {
        Self {
            inner: Box::new(stream),
        }
    }
}

impl From<TokioSocket> for TcpRelayStream {
    fn from(socket: TokioSocket) -> Self {
        Self::new(socket)
    }
}

impl AsyncSocket for TcpRelayStream {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        tokio::io::AsyncReadExt::read(&mut self.inner, buf).await
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::write_all(&mut self.inner, buf).await?;
        tokio::io::AsyncWriteExt::flush(&mut self.inner).await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::shutdown(&mut self.inner).await
    }
}

impl AsyncRead for TcpRelayStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl AsyncWrite for TcpRelayStream {
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

#[cfg(feature = "inbound-mixed")]
#[derive(Debug)]
pub(crate) struct PrefixedSocket {
    prefix: Vec<u8>,
    offset: usize,
    inner: TokioSocket,
}

#[cfg(feature = "inbound-mixed")]
impl PrefixedSocket {
    pub(crate) fn from_byte(inner: TokioSocket, first: u8) -> Self {
        Self {
            prefix: vec![first],
            offset: 0,
            inner,
        }
    }
}

#[cfg(feature = "inbound-mixed")]
impl ClientStream for PrefixedSocket {
    #[cfg(feature = "inbound-socks5")]
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

#[cfg(feature = "inbound-mixed")]
impl AsyncSocket for PrefixedSocket {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        if self.offset < self.prefix.len() {
            let available = self.prefix.len() - self.offset;
            let to_copy = cmp::min(available, buf.len());
            buf[..to_copy].copy_from_slice(&self.prefix[self.offset..self.offset + to_copy]);
            self.offset += to_copy;
            return Ok(to_copy);
        }

        self.inner.read(buf).await
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.inner.write_all(buf).await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.inner.shutdown().await
    }
}

#[cfg(feature = "inbound-mixed")]
impl AsyncRead for PrefixedSocket {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.offset < self.prefix.len() {
            let available = self.prefix.len() - self.offset;
            let to_copy = cmp::min(available, buf.remaining());
            if to_copy > 0 {
                let start = self.offset;
                let end = start + to_copy;
                buf.put_slice(&self.prefix[start..end]);
                self.offset = end;
            }
            return Poll::Ready(Ok(()));
        }

        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

#[cfg(feature = "inbound-mixed")]
impl AsyncWrite for PrefixedSocket {
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
