//! Shadowsocks inbound: listener lifecycle, TCP pipe entry, and UDP pipe entry.

use std::net::SocketAddr;
use std::sync::Arc;

use async_trait::async_trait;
use shadowsocks::{
    ShadowsocksAeadStream, ShadowsocksInbound, ShadowsocksInboundProfile,
    ShadowsocksInboundTcpState,
};
use tokio::net::UdpSocket;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

mod udp;

#[derive(Debug)]
pub(crate) struct ShadowsocksInboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: ShadowsocksInboundProfile,
}

#[derive(Clone)]
pub(crate) struct ShadowsocksInboundHandler {
    ss_inbound: ShadowsocksInbound,
    profile: ShadowsocksInboundProfile,
    tcp_state: ShadowsocksInboundTcpState,
}

#[async_trait]
impl InboundProtocol for ShadowsocksInboundHandler {
    type ClientStream = ShadowsocksAeadStream<TcpRelayStream>;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let mut metered = MeteredStream::new(stream);
        let accept = self
            .profile
            .accept_request(&self.ss_inbound, &mut metered)
            .await?;

        self.tcp_state.check_accept_replay(&accept)?;

        let mut session = accept.session.clone();
        let mut sa = zero_core::SessionAuth::new("shadowsocks");
        sa.principal_key = Some(self.profile.principal_key());
        session.apply_auth(sa);

        let client = self
            .profile
            .into_aead_stream(accept, metered.into_inner())?;

        Ok((session, client))
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
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let ShadowsocksInboundRequest { inbound, profile } = request;

    let local_addr = listener.local_addr()?;

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
        ss_inbound: ShadowsocksInbound,
        profile: profile.clone(),
        tcp_state: profile.tcp_state(),
    };

    let mut connections: JoinSet<Result<(), EngineError>> = JoinSet::new();

    info!(
        inbound_tag = %inbound.tag,
        protocol = "shadowsocks",
        cipher = %profile.cipher_name(),
        listen = %local_addr,
        udp = udp_socket.is_some(),
        "inbound listener ready"
    );

    if let Some(udp) = udp_socket.as_ref() {
        let engine = proxy.clone();
        let tag = inbound.tag.clone();
        let profile = profile.clone();
        let udp = udp.clone();
        connections.spawn(async move { engine.ss_udp_relay_loop(udp, &tag, profile).await });
    }

    loop {
        select! {
            changed = shutdown.changed() => {
                match changed {
                    Ok(()) if *shutdown.borrow() => break,
                    Ok(()) => {}
                    Err(_) => break,
                }
            }
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((stream, remote_addr)) => {
                        let engine = proxy.clone();
                        let tag = inbound.tag.clone();
                        let handler = handler.clone();
                        let source_addr = remote_addr_to_socket(remote_addr);
                        connections.spawn(async move {
                            match handler.accept(stream.into()).await {
                                Ok((session, client)) => {
                                    let _ = serve_inbound(
                                        &engine, session, client, &handler,
                                        &tag, source_addr,
                                    ).await;
                                }
                                Err(error) => {
                                    log_listener_connection_error(
                                        "shadowsocks", &tag, &remote_addr, &error,
                                    );
                                }
                            }
                            Ok(())
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "shadowsocks: accept error");
                        break;
                    }
                }
            }
            result = connections.join_next(), if !connections.is_empty() => {
                match result {
                    Some(Err(error)) if !error.is_cancelled() => {
                        error!(error = %error, "shadowsocks connection task panicked");
                    }
                    _ => {}
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "shadowsocks shutdown error");
            }
        }
    }

    info!(inbound_tag = %inbound.tag, protocol = "shadowsocks", "listener stopped");
    Ok(())
}

fn remote_addr_to_socket(addr: Option<zero_traits::IpAddress>) -> Option<SocketAddr> {
    addr.map(|ip| match ip {
        zero_traits::IpAddress::V4(octets) => {
            SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)), 0)
        }
        zero_traits::IpAddress::V6(octets) => {
            SocketAddr::new(std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)), 0)
        }
    })
}
