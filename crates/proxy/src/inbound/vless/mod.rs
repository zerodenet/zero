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

use super::super::logging::log_listener_connection_error;
use super::super::runtime::{bind_listener, Proxy};
use super::super::transport::{accept_ws, build_tls_acceptor, InboundTlsStream, PrefixedSocket};
use crate::transport::{relay_bidirectional_metered, ClientStream, MeteredStream, TcpRelayStream};
use async_trait::async_trait;
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};

// ── Handler (TCP path only) ─────────────────────────────────────────────

#[derive(Clone)]
struct VlessInboundHandler {
    vless_inbound: vless::VlessInbound,
}

#[async_trait]
impl InboundProtocol for VlessInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        _stream: TcpRelayStream,
    ) -> Result<(zero_core::Session, Self::ClientStream), EngineError> {
        // VLESS accept is handled inline by the listener (complex dispatch).
        Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "VLESS accept handled by listener",
        )))
    }

    async fn send_ok(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        self.vless_inbound
            .send_response(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = AsyncSocket::shutdown(client).await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = AsyncSocket::shutdown(client).await;
        Ok(())
    }
    // relay uses default
}

mod fallback;
mod helpers;
mod listener;
mod mux;
mod session;
mod udp_session;

pub(crate) use helpers::{
    encode_vless_mux_udp_response, upgrade_vless_reality_server, ConfiguredVlessUsers,
    RecordingStream,
};
