use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use zero_platform_tokio::ClientStream;
use zero_traits::AsyncSocket;

pub struct InboundTlsStream<IO = TcpStream> {
    inner: TlsStream<IO>,
}

impl InboundTlsStream {
    pub fn new(inner: TlsStream<TcpStream>) -> Self {
        Self { inner }
    }
}

impl<IO> InboundTlsStream<IO> {
    pub fn new_generic(inner: TlsStream<IO>) -> Self {
        Self { inner }
    }
}

impl<IO> ClientStream for InboundTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "not available"))
    }
}

impl<IO> AsyncSocket for InboundTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        AsyncReadExt::read(&mut self.inner, buf).await
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        AsyncWriteExt::write_all(&mut self.inner, buf).await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        AsyncWriteExt::shutdown(&mut self.inner).await
    }
}

impl<IO> AsyncRead for InboundTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_read(cx, buf)
    }
}

impl<IO> AsyncWrite for InboundTlsStream<IO>
where
    IO: AsyncRead + AsyncWrite + Unpin,
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
