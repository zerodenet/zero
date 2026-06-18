use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{error, info, warn};
use vless::build_udp_packet;
use vless::RealityServerOptions;
use vless::{VlessUser, VlessUserStore};
use zero_config::{InboundRealityConfig, VlessUserConfig};
use zero_platform_tokio::TokioSocket;
use zero_traits::AsyncSocket;

use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput, UdpPipe, UdpPipeInput};
use crate::runtime::udp_associate::helpers::{
    log_completed_udp_flow, recv_upstream_packet, wait_for_upstream_idle,
};
use crate::runtime::udp_dispatch::UdpDispatch;

use crate::logging::log_listener_connection_error;
use crate::runtime::{bind_listener, Proxy};
use crate::transport::{accept_ws, build_tls_acceptor, InboundTlsStream, PrefixedSocket};
use crate::transport::{relay_bidirectional_metered, ClientStream, MeteredStream, TcpRelayStream};
use async_trait::async_trait;
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};

use super::*;

impl Proxy {
    pub(crate) async fn relay_fallback_no_tls(
        &self,
        client: impl AsyncRead + AsyncWrite + Unpin + Send + 'static,
        upstream: TokioSocket,
    ) -> Result<(), EngineError> {
        let metered_client = MeteredStream::new(client);
        let metered_upstream = MeteredStream::new(upstream);
        let result =
            relay_bidirectional_metered(metered_client, metered_upstream, |_| {}, |_| {}).await;
        match result {
            Ok(_) => Ok(()),
            Err(e)
                if e.kind() == io::ErrorKind::NotConnected
                    || e.kind() == io::ErrorKind::BrokenPipe =>
            {
                Ok(())
            }
            Err(e) => Err(EngineError::Io(e)),
        }
    }

    /// Relay to fallback: replay captured VLESS header bytes, then relay.
    pub(crate) async fn relay_fallback<S>(
        &self,
        client_stream: S,
        head: Vec<u8>,
        fallback: &zero_config::FallbackConfig,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let mut upstream = self
            .protocols
            .direct_outbound
            .connect_host(&fallback.server, fallback.port, self.resolver.as_ref())
            .await?;

        if !head.is_empty() {
            tokio::io::AsyncWriteExt::write_all(&mut upstream, &head).await?;
        }

        let metered_client = MeteredStream::new(client_stream);
        let metered_upstream = MeteredStream::new(upstream);

        let result =
            relay_bidirectional_metered(metered_client, metered_upstream, |_| {}, |_| {}).await;

        match result {
            Ok(_) => Ok(()),
            Err(e)
                if e.kind() == io::ErrorKind::NotConnected
                    || e.kind() == io::ErrorKind::BrokenPipe =>
            {
                Ok(())
            }
            Err(e) => Err(EngineError::Io(e)),
        }
    }
}
