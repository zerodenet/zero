//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use async_trait::async_trait;
use vmess::{VmessInbound, VmessInboundProfile};
use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::InboundProtocol;
use crate::transport::TcpRelayStream;

// Trait-based handler (raw TLS path).

#[derive(Clone)]
pub(crate) struct VmessInboundHandler {
    vmess_inbound: VmessInbound,
    profile: VmessInboundProfile,
    tls_acceptor: crate::transport::TlsAcceptor,
}

#[async_trait]
impl InboundProtocol for VmessInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let _ = stream;
        unreachable!("vmess accept is handled before serve_inbound dispatch")
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
}

// Handler for transport-wrapped connections (WS/gRPC).
// Only send_ok / send_blocked / send_upstream_failure are used by serve_inbound;
// accept is unreachable because the protocol was already authenticated.

#[derive(Clone)]
pub(crate) struct VmessTransportHandler;

#[async_trait]
impl InboundProtocol for VmessTransportHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        _stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        unreachable!("accept handled in listener transport dispatch")
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
}

// Listener.

mod listener;
pub(crate) mod model;
mod mux;
mod mux_udp;
mod transport;
mod udp_session;

pub(crate) use listener::run_vmess_listener_with_bound;
pub(crate) use transport::{handle_vmess_grpc, handle_vmess_raw, handle_vmess_ws};
