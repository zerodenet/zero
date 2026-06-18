//! VMess inbound: TLS accept, transport dispatch (WS/gRPC), protocol auth, route, TCP relay.

use super::*;

use std::io;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::mpsc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::{error, info, warn};
use vmess::{VmessAccept, VmessAeadStream, VmessCipher, VmessInbound, VmessUser};
use zero_config::{GrpcConfig, InboundConfig, WebSocketConfig};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

use crate::runtime::bind_listener;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_associate::helpers::{
    log_completed_udp_flow, recv_upstream_packet, wait_for_upstream_idle,
};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

/// `AsyncSocket` for a rustls TLS stream over TcpRelayStream.

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
            if session.network == Network::Udp {
                proxy.run_vmess_udp_relay(client, session, tag).await
            } else if vmess::is_mux_cool_session(&session) {
                proxy.run_vmess_mux_session(client, tag).await
            } else {
                serve_inbound(proxy, session, client, handler, tag, source_addr).await
            }
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

    let accepted = if handler.users.len() == 1 {
        handler
            .vmess_inbound
            .accept_tcp(&mut ws, &handler.users[0])
            .await?
    } else {
        handler
            .vmess_inbound
            .accept_tcp_multi(&mut ws, &handler.users)
            .await?
    };
    let session = accepted.session.clone();
    let client = wrap_vmess_client(TcpRelayStream::new(ws), accepted)?;

    let transport_handler = VmessTransportHandler;
    if session.network == Network::Udp {
        proxy.run_vmess_udp_relay(client, session, tag).await
    } else if vmess::is_mux_cool_session(&session) {
        proxy.run_vmess_mux_session(client, tag).await
    } else {
        serve_inbound(proxy, session, client, &transport_handler, tag, source_addr).await
    }
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
    let users = handler.users.clone();
    let vmess = handler.vmess_inbound;
    let proxy = proxy.clone();
    let tag = tag.to_owned();

    crate::transport::serve_grpc(tls, &service_names, move |mut grpc_stream| {
        let users = users.clone();
        let proxy = proxy.clone();
        let tag = tag.clone();
        async move {
            let result = if users.len() == 1 {
                vmess.accept_tcp(&mut grpc_stream, &users[0]).await
            } else {
                vmess.accept_tcp_multi(&mut grpc_stream, &users).await
            };
            match result {
                Ok(accepted) => {
                    let session = accepted.session.clone();
                    let client = wrap_vmess_client(TcpRelayStream::new(grpc_stream), accepted)?;
                    let transport_handler = VmessTransportHandler;
                    if session.network == Network::Udp {
                        proxy.run_vmess_udp_relay(client, session, &tag).await
                    } else if vmess::is_mux_cool_session(&session) {
                        proxy.run_vmess_mux_session(client, &tag).await
                    } else {
                        serve_inbound(
                            &proxy,
                            session,
                            client,
                            &transport_handler,
                            &tag,
                            source_addr,
                        )
                        .await
                    }
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
