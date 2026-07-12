//! Shadowsocks inbound: listener lifecycle, TCP pipe entry, and UDP pipe entry.

use std::sync::Arc;

use tokio::net::UdpSocket;
use tokio::sync::watch;
use tracing::warn;
use zero_config::InboundConfig;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, NoClientResponseStreamProtocol};
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

#[path = "udp.rs"]
mod udp;

pub(crate) async fn handle_shadowsocks_connection(
    proxy: &Proxy,
    inbound_tag: &str,
    source_addr: Option<std::net::SocketAddr>,
    stream: TcpRelayStream,
    acceptor: &zero_transport::shadowsocks_transport::OwnedShadowsocksInboundTcpAcceptor,
) {
    if let Err(error) = acceptor
        .accept_and_dispatch_stream(MeteredStream::new(stream), |session, client| async move {
            let protocol = NoClientResponseStreamProtocol::new();
            let _ =
                serve_inbound(proxy, session, client, &protocol, inbound_tag, source_addr).await;
            Ok::<(), EngineError>(())
        })
        .await
    {
        log_listener_connection_error("shadowsocks", inbound_tag, &source_addr, &error);
    }
}

pub(crate) async fn run_shadowsocks_listener_with_bound(
    proxy: &Proxy,
    inbound: InboundConfig,
    profile: zero_transport::shadowsocks_transport::OwnedShadowsocksInboundProfile,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let (acceptor, udp_session) = profile.into_listener_bindings().into_parts();

    let udp_socket = match UdpSocket::bind(&format!(
        "{}:{}",
        inbound.listen.address, inbound.listen.port
    ))
    .await
    {
        Ok(s) => Some(Arc::new(s)),
        Err(e) => {
            warn!(error = %e, "shadowsocks: failed to bind UDP socket, UDP disabled");
            None
        }
    };

    let udp_task = udp_socket.as_ref().map(|udp| {
        let engine = proxy.clone();
        let tag = inbound.tag.clone();
        let udp = udp.clone();
        tokio::spawn(async move {
            if let Err(error) = udp::ss_udp_relay_loop(&engine, udp, &tag, udp_session).await {
                warn!(%error, "shadowsocks UDP relay stopped");
            }
        })
    });

    let result = run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "shadowsocks",
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let acceptor = acceptor.clone();
            async move {
                handle_shadowsocks_connection(
                    &engine,
                    &tag,
                    source_addr,
                    TcpRelayStream::from(stream),
                    &acceptor,
                )
                .await;
            }
        },
    })
    .await;

    if let Some(udp) = udp_socket.as_ref() {
        drop(udp.clone());
    }
    if let Some(task) = udp_task {
        task.abort();
        let _ = task.await;
    }

    result
}
