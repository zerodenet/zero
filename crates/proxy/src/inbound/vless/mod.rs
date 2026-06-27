use std::io;

use async_trait::async_trait;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::inbound_protocol::InboundProtocol;
use crate::transport::TcpRelayStream;

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
pub(crate) mod model;
mod mux;
mod session;
mod udp_session;

pub(crate) use listener::run_vless_listener_with_bound;

pub(crate) use helpers::{
    upgrade_vless_reality_server, ConfiguredVlessUser, ConfiguredVlessUsers, RecordingStream,
};
