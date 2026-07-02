//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use std::io;

use tokio::io::{AsyncRead, AsyncWrite};
use tracing::warn;
use zero_config::{GrpcConfig, WebSocketConfig};
use zero_core::{Session, SessionAuth};
use zero_engine::EngineError;

use super::{VmessInboundHandler, VmessTransportHandler};
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{AsyncSocketStream, TcpRelayStream};

struct VmessAcceptedStreamBridge<'a, H> {
    proxy: &'a Proxy,
    handler: &'a H,
    tag: &'a str,
    source_addr: Option<std::net::SocketAddr>,
}

impl<S, H> vmess::mux::VmessInboundAcceptedStreamDispatcher<S> for VmessAcceptedStreamBridge<'_, H>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    H: InboundProtocol<ClientStream = TcpRelayStream>,
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
            self.handler,
            self.tag,
            self.source_addr,
        )
        .await
    }

    async fn dispatch_udp_stream(
        &mut self,
        session: Session,
        stream: S,
        responder: vmess::VmessInboundUdpResponder,
        auth: Option<SessionAuth>,
    ) -> Result<(), Self::Error> {
        self.proxy
            .run_vmess_udp_relay(
                TcpRelayStream::new(stream),
                session,
                responder,
                auth,
                self.tag,
            )
            .await
    }

    async fn dispatch_mux_stream(
        &mut self,
        reader: tokio::io::ReadHalf<S>,
        mux_server: vmess::mux::VmessInboundMuxServer,
    ) -> Result<(), Self::Error> {
        self.proxy
            .run_vmess_mux_session(reader, mux_server, self.tag)
            .await
    }
}

async fn dispatch_vmess_client<S, H>(
    proxy: &Proxy,
    client: vmess::mux::VmessInboundAcceptedStream<S>,
    handler: &H,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    H: InboundProtocol<ClientStream = TcpRelayStream>,
{
    let mut bridge = VmessAcceptedStreamBridge {
        proxy,
        handler,
        tag,
        source_addr,
    };
    client.dispatch_with(&mut bridge).await
}

/// Raw TLS path: TLS accept -> VMess auth -> serve_inbound.
pub(crate) async fn handle_vmess_raw(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = handler
        .tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(e)))?;
    let client = handler
        .profile
        .accept_client(handler.vmess_inbound, AsyncSocketStream::new(tls))
        .await?;
    dispatch_vmess_client(proxy, client, handler, tag, source_addr).await
}

/// WebSocket path: TLS accept -> WS upgrade -> VMess auth -> serve_inbound.
pub(crate) async fn handle_vmess_ws(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    ws_cfg: &WebSocketConfig,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = handler
        .tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(e)))?;

    let ws = crate::transport::accept_ws(tls, &ws_cfg.path).await?;

    let client = handler
        .profile
        .accept_client(handler.vmess_inbound, TcpRelayStream::new(ws))
        .await?;

    let transport_handler = VmessTransportHandler;
    dispatch_vmess_client(proxy, client, &transport_handler, tag, source_addr).await
}

/// gRPC path: TLS accept -> serve_grpc -> per-stream VMess auth -> serve_inbound.
pub(crate) async fn handle_vmess_grpc(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    grpc_cfg: &GrpcConfig,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let tls = handler
        .tls_acceptor
        .accept(stream)
        .await
        .map_err(|e| EngineError::Io(io::Error::other(e)))?;

    let service_names = grpc_cfg.service_names.clone();
    let profile = handler.profile.clone();
    let vmess = handler.vmess_inbound;
    let proxy = proxy.clone();
    let tag = tag.to_owned();

    crate::transport::serve_grpc(tls, &service_names, move |grpc_stream| {
        let profile = profile.clone();
        let proxy = proxy.clone();
        let tag = tag.clone();
        async move {
            let result = profile
                .accept_client(vmess, TcpRelayStream::new(grpc_stream))
                .await;
            match result {
                Ok(client) => {
                    let transport_handler = VmessTransportHandler;
                    dispatch_vmess_client(&proxy, client, &transport_handler, &tag, source_addr)
                        .await
                }
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
