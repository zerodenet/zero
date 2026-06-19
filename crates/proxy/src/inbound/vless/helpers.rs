use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use vless::RealityServerOptions;
use vless::{VlessUser, VlessUserStore};
use zero_config::{InboundRealityConfig, VlessUserConfig};
use zero_traits::AsyncSocket;

use crate::transport::ClientStream;

/// Encode a VLESS MUX UDP response: build a VLESS UDP packet and wrap it
/// as a MUX data frame for the given session ID.
pub(crate) fn encode_vless_mux_udp_response(
    mux_session_id: u16,
    target: &zero_core::Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    let udp_packet = vless::build_udp_packet(target, port, payload)?;
    Ok(vless::encode_data_frame(mux_session_id, &udp_packet))
}

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

pub(crate) struct ConfiguredVlessUsers<'a> {
    pub(crate) users: &'a [VlessUserConfig],
}

impl VlessUserStore for ConfiguredVlessUsers<'_> {
    fn find_user(&self, id: &[u8; 16]) -> Option<VlessUser> {
        self.users.iter().find_map(|user| {
            let configured_id = vless::parse_uuid(&user.id).ok()?;
            if &configured_id == id {
                let flow = user.flow.as_deref().and_then(|f| vless::parse_flow(f).ok());
                Some(VlessUser {
                    credential_id: user.credential_id.clone(),
                    principal_key: user.principal_key.clone(),
                    up_bps: user.up_bps,
                    down_bps: user.down_bps,
                    flow,
                })
            } else {
                None
            }
        })
    }
}
