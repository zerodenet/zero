use std::cmp;
use std::io;
use std::net::SocketAddr;

use zero_platform_tokio::TokioSocket;
use zero_traits::AsyncSocket;

pub(crate) trait ClientStream: AsyncSocket<Error = io::Error> + Send + Sync + Unpin {
    fn into_tokio_socket(self) -> TokioSocket;
    fn local_addr(&self) -> io::Result<SocketAddr>;
}

impl ClientStream for TokioSocket {
    fn into_tokio_socket(self) -> TokioSocket {
        self
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.local_addr()
    }
}

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
}

impl ClientStream for PrefixedSocket {
    fn into_tokio_socket(self) -> TokioSocket {
        self.inner
    }

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
