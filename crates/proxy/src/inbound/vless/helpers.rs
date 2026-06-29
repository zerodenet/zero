use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use vless::RealityServerOptions;
use zero_config::InboundRealityConfig;
use zero_traits::AsyncSocket;

use crate::transport::ClientStream;

// ── Fallback helpers ──

/// Wraps an inner stream and records all bytes read, for replay to a
/// fallback target when VLESS authentication fails.
pub(crate) struct RecordingStream<S> {
    inner: S,
    recorded: Vec<u8>,
}

impl<S> RecordingStream<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self {
            inner,
            recorded: Vec::with_capacity(128),
        }
    }
    pub(crate) fn into_parts(self) -> (S, Vec<u8>) {
        (self.inner, self.recorded)
    }
}

impl<S> AsyncRead for RecordingStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let prev = buf.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &result {
            let n = buf.filled().len() - prev;
            if n > 0 {
                self.recorded.extend_from_slice(&buf.filled()[prev..]);
            }
        }
        result
    }
}

impl<S> AsyncWrite for RecordingStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

impl<S> AsyncSocket for RecordingStream<S>
where
    S: AsyncSocket<Error = io::Error> + Send + Sync,
{
    type Error = io::Error;
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let n = self.inner.read(buf).await?;
        self.recorded.extend_from_slice(&buf[..n]);
        Ok(n)
    }
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.inner.write_all(buf).await
    }
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.inner.shutdown().await
    }
}

impl<S> ClientStream for RecordingStream<S>
where
    S: ClientStream + Send + Sync,
{
    fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }
}

pub(crate) async fn upgrade_vless_reality_server<S>(
    stream: S,
    reality: &InboundRealityConfig,
) -> std::io::Result<vless::RealityTlsStream<S>>
where
    S: ClientStream + 'static,
{
    let server_name = reality.server_name.as_deref().unwrap_or("localhost");
    vless::upgrade_reality_server(
        stream,
        RealityServerOptions {
            private_key: &reality.private_key,
            short_ids: &reality.short_ids,
            server_name,
            cipher_suites: &reality.cipher_suites,
        },
    )
    .await
}
