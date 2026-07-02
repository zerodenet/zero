//! Shadowsocks inbound: listener lifecycle, TCP pipe entry, and UDP pipe entry.

use std::sync::Arc;

use async_trait::async_trait;
use shadowsocks::{
    ShadowsocksAeadStream, ShadowsocksInboundProfile, ShadowsocksInboundTcpAcceptor,
};
use tokio::net::UdpSocket;
use tokio::sync::watch;
use tracing::warn;
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

mod udp;

pub(crate) struct ShadowsocksInboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: ShadowsocksInboundProfile,
    pub(crate) udp_session: shadowsocks::udp::ShadowsocksInboundAcceptedUdpSession,
}

#[derive(Clone)]
pub(crate) struct ShadowsocksInboundHandler {
    acceptor: ShadowsocksInboundTcpAcceptor,
}

#[async_trait]
impl InboundProtocol for ShadowsocksInboundHandler {
    type ClientStream = ShadowsocksAeadStream<MeteredStream<TcpRelayStream>>;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let metered = MeteredStream::new(stream);
        self.acceptor
            .accept_stream(metered)
            .await
            .map_err(EngineError::from)
    }

    async fn send_ok(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(()) // Shadowsocks has no success response
    }

    async fn send_blocked(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(
        &self,
        _client: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        Ok(())
    }
}

pub(crate) async fn run_shadowsocks_listener_with_bound(
    proxy: &Proxy,
    request: ShadowsocksInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let ShadowsocksInboundRequest {
        inbound,
        profile,
        udp_session,
    } = request;

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

    let handler = ShadowsocksInboundHandler {
        acceptor: ShadowsocksInboundTcpAcceptor::new(profile.clone()),
    };

    let udp_task = udp_socket.as_ref().map(|udp| {
        let engine = proxy.clone();
        let tag = inbound.tag.clone();
        let udp = udp.clone();
        tokio::spawn(async move {
            if let Err(error) = engine.ss_udp_relay_loop(udp, &tag, udp_session).await {
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
            let handler = handler.clone();
            async move {
                match handler.accept(stream.into()).await {
                    Ok((session, client)) => {
                        let _ =
                            serve_inbound(&engine, session, client, &handler, &tag, source_addr)
                                .await;
                    }
                    Err(error) => {
                        log_listener_connection_error("shadowsocks", &tag, &source_addr, &error);
                    }
                }
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
