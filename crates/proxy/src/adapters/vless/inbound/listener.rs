use async_trait::async_trait;
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::InboundProtocol;
use crate::transport::TcpRelayStream;

// TCP runtime bridge: VLESS accept happens in session glue before serve_inbound.

#[async_trait]
impl InboundProtocol for vless::VlessInbound {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        _stream: TcpRelayStream,
    ) -> Result<(zero_core::Session, Self::ClientStream), EngineError> {
        unreachable!("vless accept is handled before serve_inbound dispatch")
    }

    async fn send_ok(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        vless::VlessInbound::send_ok(self, client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        vless::VlessInbound::send_blocked(self, client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_upstream_failure(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        vless::VlessInbound::send_upstream_failure(self, client)
            .await
            .map_err(EngineError::from)
    }
    // relay uses default
}

mod fallback;
mod helpers;
mod listener;
pub(crate) mod model;
mod mux;
mod mux_udp;
mod session;
mod udp_session;

pub(crate) use listener::run_vless_listener_with_bound;

pub(crate) use helpers::upgrade_vless_reality_server;
