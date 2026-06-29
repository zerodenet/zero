//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use std::io;

use async_trait::async_trait;
use vmess::{VmessInbound, VmessInboundProfile};
use zero_core::Session;
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::inbound_protocol::InboundProtocol;
use crate::transport::TcpRelayStream;

/// `AsyncSocket` for a rustls TLS stream over TcpRelayStream.
struct TlsStream(tokio_rustls::server::TlsStream<TcpRelayStream>);

impl AsyncSocket for TlsStream {
    type Error = io::Error;
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        tokio::io::AsyncReadExt::read(&mut self.0, buf).await
    }
    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::write_all(&mut self.0, buf).await
    }
    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        tokio::io::AsyncWriteExt::shutdown(&mut self.0).await
    }
}

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
        let tls = self
            .tls_acceptor
            .accept(stream)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(e)))?;
        let mut sock = TlsStream(tls);
        let accepted = self
            .profile
            .accept_tcp(self.vmess_inbound, &mut sock)
            .await?;
        let session = accepted.session.clone();
        let client = TcpRelayStream::new(vmess::wrap_tcp_inbound_stream(
            TcpRelayStream::new(sock.0),
            accepted,
        )?);
        Ok((session, client))
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
mod transport;

pub(crate) use listener::run_vmess_listener_with_bound;
pub(crate) use transport::{handle_vmess_grpc, handle_vmess_raw, handle_vmess_ws};
