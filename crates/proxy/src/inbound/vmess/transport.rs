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
    match vmess::mux::classify_inbound_session(&session) {
        vmess::mux::VmessInboundSessionKind::Udp => {
            proxy.run_vmess_udp_relay(client, session, tag).await
        }
        vmess::mux::VmessInboundSessionKind::Mux => proxy.run_vmess_mux_session(client, tag).await,
        vmess::mux::VmessInboundSessionKind::Tcp => {
            serve_inbound(proxy, session, client, handler, tag, source_addr).await
        }
    }
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
