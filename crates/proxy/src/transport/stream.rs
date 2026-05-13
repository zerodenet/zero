use std::cmp;
use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zero_platform_tokio::TokioSocket;
use zero_traits::AsyncSocket;

use std::pin::Pin;
use std::task::{Context, Poll};

pub(crate) use zero_platform_tokio::{ClientStream, TcpRelayStream};

#[derive(Debug)]
pub(crate) struct PrefixedSocket {
    prefix: Vec<u8>,
    offset: usize,
    inner: TokioSocket,
}

impl PrefixedSocket {
    
    pub(crate) fn from_byte(inner: TokioSocket, first: u8) -> Self {
        Self {
            prefix: vec![first],
            offset: 0,
            inner,
        }
    }

    pub(crate) fn from_prefix(inner: TokioSocket, prefix: Vec<u8>) -> Self {
        Self {
            prefix,
            offset: 0,
            inner,
        }
    }
}
impl ClientStream for PrefixedSocket {
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}


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
