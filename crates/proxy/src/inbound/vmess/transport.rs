//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use std::io;

use tracing::warn;
use zero_config::{GrpcConfig, WebSocketConfig};
use zero_core::Session;
use zero_engine::EngineError;

use super::{VmessInboundHandler, VmessTransportHandler};
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

struct VmessAcceptedSessionHandler<'a, H> {
    proxy: &'a Proxy,
    session: Option<Session>,
    client: Option<TcpRelayStream>,
    handler: &'a H,
    tag: &'a str,
    source_addr: Option<std::net::SocketAddr>,
}

impl<H> vmess::mux::VmessInboundSessionHandler for VmessAcceptedSessionHandler<'_, H>
where
    H: InboundProtocol<ClientStream = TcpRelayStream>,
{
    type Error = EngineError;

    async fn handle_tcp_session(&mut self) -> Result<(), Self::Error> {
        serve_inbound(
            self.proxy,
            self.session
                .take()
                .expect("vmess accepted session is dispatched once"),
            self.client
                .take()
                .expect("vmess accepted client is dispatched once"),
            self.handler,
            self.tag,
            self.source_addr,
        )
        .await
    }

    async fn handle_udp_session(&mut self) -> Result<(), Self::Error> {
        self.proxy
            .run_vmess_udp_relay(
                self.client
                    .take()
                    .expect("vmess accepted client is dispatched once"),
                self.session
                    .take()
                    .expect("vmess accepted session is dispatched once"),
                self.tag,
            )
            .await
    }

    async fn handle_mux_session(&mut self) -> Result<(), Self::Error> {
        self.proxy
            .run_vmess_mux_session(
                self.client
                    .take()
                    .expect("vmess accepted client is dispatched once"),
                self.tag,
            )
            .await
    }
}

async fn dispatch_vmess_session<H>(
    proxy: &Proxy,
    session: Session,
    client: TcpRelayStream,
    handler: &H,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError>
where
    H: InboundProtocol<ClientStream = TcpRelayStream>,
{
    let dispatch_session = session.clone();
    let mut session_handler = VmessAcceptedSessionHandler {
        proxy,
        session: Some(session),
        client: Some(client),
        handler,
        tag,
        source_addr,
    };
    vmess::mux::dispatch_inbound_session(&dispatch_session, &mut session_handler).await
}

/// Raw TLS path: TLS accept -> VMess auth -> serve_inbound.
pub(crate) async fn handle_vmess_raw(
    proxy: &Proxy,
    handler: &VmessInboundHandler,
    stream: TcpRelayStream,
    tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    match handler.accept(stream).await {
        Ok((session, client)) => {
            dispatch_vmess_session(proxy, session, client, handler, tag, source_addr).await
        }
        Err(e) => Err(e),
    }
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

    let mut ws = crate::transport::accept_ws(tls, &ws_cfg.path).await?;

    let accepted = handler
        .profile
        .accept_tcp(handler.vmess_inbound, &mut ws)
        .await?;
    let session = accepted.session.clone();
    let client = TcpRelayStream::new(vmess::wrap_tcp_inbound_stream(
        TcpRelayStream::new(ws),
        accepted,
    )?);

    let transport_handler = VmessTransportHandler;
    dispatch_vmess_session(proxy, session, client, &transport_handler, tag, source_addr).await
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

    crate::transport::serve_grpc(tls, &service_names, move |mut grpc_stream| {
        let profile = profile.clone();
        let proxy = proxy.clone();
        let tag = tag.clone();
        async move {
            let result = profile.accept_tcp(vmess, &mut grpc_stream).await;
            match result {
                Ok(accepted) => {
                    let session = accepted.session.clone();
                    let client = TcpRelayStream::new(vmess::wrap_tcp_inbound_stream(
                        TcpRelayStream::new(grpc_stream),
                        accepted,
                    )?);
                    let transport_handler = VmessTransportHandler;
                    dispatch_vmess_session(
                        &proxy,
                        session,
                        client,
                        &transport_handler,
                        &tag,
                        source_addr,
                    )
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
