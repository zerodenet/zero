use zero_traits::AsyncSocket;

use super::stream::ClientStream;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StreamTraffic {
    pub(crate) read_bytes: u64,
    pub(crate) written_bytes: u64,
}

impl StreamTraffic {
    pub(crate) fn is_empty(self) -> bool {
        self.read_bytes == 0 && self.written_bytes == 0
    }
}

#[derive(Debug)]
pub(crate) struct MeteredStream<S> {
    inner: S,
    traffic: StreamTraffic,
}

impl<S> MeteredStream<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self {
            inner,
            traffic: StreamTraffic::default(),
        }
    }

    pub(crate) fn drain_traffic(&mut self) -> StreamTraffic {
        let traffic = self.traffic;
        self.traffic = StreamTraffic::default();
        traffic
    }

    pub(crate) fn into_inner(self) -> S {
        self.inner
    }
}

impl<S> AsyncSocket for MeteredStream<S>
where
    S: AsyncSocket,
{
    type Error = S::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let read = self.inner.read(buf).await?;
        self.traffic.read_bytes = self.traffic.read_bytes.saturating_add(read as u64);
        Ok(read)
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.inner.write_all(buf).await?;
        self.traffic.written_bytes = self.traffic.written_bytes.saturating_add(buf.len() as u64);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.inner.shutdown().await
    }
}

impl<S> ClientStream for MeteredStream<S>
where
    S: ClientStream,
{
    fn into_tokio_socket(self) -> zero_platform_tokio::TokioSocket {
        self.inner.into_tokio_socket()
    }

    fn local_addr(&self) -> std::io::Result<std::net::SocketAddr> {
        self.inner.local_addr()
    }
}
