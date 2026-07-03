//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use std::io;

use tokio::io::{AsyncRead, AsyncWrite};
use tracing::warn;
use zero_config::{GrpcConfig, WebSocketConfig};
use zero_core::{Session, SessionAuth};
use zero_engine::EngineError;

use super::mux::run_vmess_mux_session;
use super::udp_session::run_vmess_udp_relay;
use crate::runtime::inbound_protocol::{serve_inbound, NoClientResponseInboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{AsyncSocketStream, TcpRelayStream};

struct VmessAcceptedStreamDispatcher<'a> {
    proxy: &'a Proxy,
    inbound_tag: &'a str,
    source_addr: Option<std::net::SocketAddr>,
}

impl<S> vmess::mux::VmessInboundAcceptedStreamDispatcher<S> for VmessAcceptedStreamDispatcher<'_>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    type Error = EngineError;

    async fn dispatch_tcp_stream(
        &mut self,
        session: Session,
        stream: S,
    ) -> Result<(), Self::Error> {
        serve_inbound(
            self.proxy,
            session,
            TcpRelayStream::new(stream),
            &NoClientResponseInboundProtocol,
            self.inbound_tag,
            self.source_addr,
        )
        .await
    }

    async fn dispatch_udp_stream(
        &mut self,
        session: Session,
        stream: S,
        responder: vmess::udp::VmessInboundUdpResponder,
        auth: Option<SessionAuth>,
    ) -> Result<(), Self::Error> {
        run_vmess_udp_relay(
            self.proxy,
            TcpRelayStream::new(stream),
            session,
            responder,
            auth,
            self.inbound_tag,
        )
        .await
    }

    async fn dispatch_mux_stream(
        &mut self,
        reader: tokio::io::ReadHalf<S>,
        mux_server: vmess::mux::VmessInboundMuxServer,
    ) -> Result<(), Self::Error> {
        run_vmess_mux_session(self.proxy, reader, mux_server, self.inbound_tag).await
    }
}

async fn dispatch_vmess_client<S>(
    proxy: &Proxy,
    client: vmess::mux::VmessInboundAcceptedStream<S>,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
{
    let mut bridge = VmessAcceptedStreamDispatcher {
        proxy,
        inbound_tag: tag,
        source_addr,
    };
    client.dispatch_with(&mut bridge).await
}

/// Raw TLS path: TLS accept -> VMess auth -> serve_inbound.
pub(crate) async fn handle_vmess_raw(
    proxy: &Proxy,
    tls_acceptor: &crate::transport::TlsAcceptor,
    profile: &vmess::VmessInboundProfile,
    stream: TcpRelayStream,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(e)))?;
    let client = profile
        .accept_client(vmess::VmessInbound, AsyncSocketStream::new(tls))
        .await?;
    dispatch_vmess_client(proxy, client, tag, source_addr).await
}

/// WebSocket path: TLS accept -> WS upgrade -> VMess auth -> serve_inbound.
pub(crate) async fn handle_vmess_ws(
    proxy: &Proxy,
    tls_acceptor: &crate::transport::TlsAcceptor,
    profile: &vmess::VmessInboundProfile,
    stream: TcpRelayStream,
    ws_cfg: &WebSocketConfig,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(e)))?;

    let ws = crate::transport::accept_ws(tls, &ws_cfg.path).await?;

    let client = profile
        .accept_client(vmess::VmessInbound, TcpRelayStream::new(ws))
        .await?;

    dispatch_vmess_client(proxy, client, tag, source_addr).await
}

/// gRPC path: TLS accept -> serve_grpc -> per-stream VMess auth -> serve_inbound.
pub(crate) async fn handle_vmess_grpc(
    proxy: &Proxy,
    tls_acceptor: &crate::transport::TlsAcceptor,
    profile: &vmess::VmessInboundProfile,
    stream: TcpRelayStream,
    grpc_cfg: &GrpcConfig,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(e)))?;

    let service_names = grpc_cfg.service_names.clone();
    let profile = profile.clone();
    let proxy = proxy.clone();
    let tag = tag.to_owned();

    crate::transport::serve_grpc(tls, &service_names, move |grpc_stream| {
        let profile = profile.clone();
        let proxy = proxy.clone();
        let tag = tag.clone();
        async move {
            let result = profile
                .accept_client(vmess::VmessInbound, TcpRelayStream::new(grpc_stream))
                .await;
            match result {
                Ok(client) => dispatch_vmess_client(&proxy, client, &tag, source_addr).await,
                Err(e) => {
                    warn!(%e, "vmess grpc auth failed");
                    Err(EngineError::Core(zero_core::Error::Protocol(
                        "vmess auth failed",
                    )))
                }
            }
        }
    })
    .await
}
