//! SOCKS5 outbound protocol implementation

use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time::Instant as TokioInstant;

use socks5::{Socks5UdpRelay, Socks5UdpRelayEndpoint, Socks5UdpRelayError};
use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::{TokioDatagramSocket, TokioSocket};

use crate::logging::{
    log_udp_upstream_association_created, log_udp_upstream_association_dropped,
    log_udp_upstream_association_reused,
};
use crate::runtime::Proxy;
use crate::transport::MeteredStream;

/// SOCKS5 UDP association close reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpstreamAssociationCloseReason {
    Closed,
    IdleTimeout,
    Dropped,
}

/// Active SOCKS5 UDP upstream association
pub struct ActiveUpstreamSocks5UdpAssociation {
    tag: String,
    server: String,
    port: u16,
    proxy: Proxy,
    close_recorded: AtomicBool,
    _control: TokioSocket,
    relay: Socks5UdpRelay<TokioDatagramSocket>,
}

/// SOCKS5 UDP association context
#[derive(Clone)]
pub struct Socks5UdpAssociation {
    pub tag: String,
    pub server: String,
    pub port: u16,
    pub auth: Option<(String, String)>,
}

impl ActiveUpstreamSocks5UdpAssociation {
    pub async fn establish(
        proxy: &Proxy,
        tag: &str,
        server: &str,
        port: u16,
        auth: Option<(&str, &str)>,
        session_id: u64,
    ) -> Result<Self, EngineError> {
        let control = proxy
            .protocols
            .direct_outbound
            .connect_host(server, port, proxy.resolver.as_ref())
            .await?;
        let mut control = MeteredStream::new(control);
        let (relay_address, relay_port) = proxy
            .protocols
            .socks5_outbound
            .establish_udp_association_with_auth(
                &mut control,
                auth.map(|(username, password)| socks5::Socks5OutboundAuth { username, password }),
            )
            .await?;
        proxy.record_session_outbound_traffic(session_id, control.drain_traffic());
        let control = control.into_inner();
        let relay_addr = proxy
            .protocols
            .direct_outbound
            .resolve_address(
                &relay_address,
                relay_port,
                proxy.resolver.as_ref(),
                "failed to resolve upstream socks5 udp relay",
            )
            .await?;

        let bind_addr = match relay_addr {
            SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED), 0),
            SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED), 0),
        };
        let relay = TokioDatagramSocket::bind_addr(bind_addr).await?;

        Ok(Self {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            proxy: proxy.clone(),
            close_recorded: AtomicBool::new(false),
            _control: control,
            relay: Socks5UdpRelay::new(
                relay,
                Socks5UdpRelayEndpoint {
                    address: zero_platform_tokio::socket_addr_to_ip(relay_addr),
                    port: relay_addr.port(),
                },
            ),
        })
    }

    pub fn matches(&self, tag: &str, server: &str, port: u16) -> bool {
        self.tag == tag && self.server == server && self.port == port
    }

    pub fn outbound_tag(&self) -> &str {
        &self.tag
    }

    pub fn upstream_endpoint(&self) -> (&str, u16) {
        (&self.server, self.port)
    }

    pub fn close(self, reason: UpstreamAssociationCloseReason) {
        self.close_recorded.store(true, Ordering::Relaxed);

        match reason {
            UpstreamAssociationCloseReason::Closed => {
                self.proxy.record_udp_upstream_association_closed();
            }
            UpstreamAssociationCloseReason::IdleTimeout => {
                self.proxy.record_udp_upstream_association_idle_timeout();
            }
            UpstreamAssociationCloseReason::Dropped => {
                self.proxy.record_udp_upstream_association_dropped();
            }
        }
    }

    pub async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        match self.relay.send_packet(target, port, payload).await {
            Ok(sent) => Ok(sent),
            Err(Socks5UdpRelayError::Socket(error)) => Err(error.into()),
            Err(Socks5UdpRelayError::Protocol(error)) => Err(error.into()),
        }
    }

    pub async fn recv_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        match self.relay.recv_packet(buf).await {
            Ok(read) => Ok(read),
            Err(Socks5UdpRelayError::Socket(error)) => Err(error.into()),
            Err(Socks5UdpRelayError::Protocol(error)) => Err(error.into()),
        }
    }
}

impl Drop for ActiveUpstreamSocks5UdpAssociation {
    fn drop(&mut self) {
        if !self.close_recorded.load(Ordering::Relaxed) {
            self.proxy.record_udp_upstream_association_closed();
            self.close_recorded.store(true, Ordering::Relaxed);
        }
    }
}

/// Ensures a SOCKS5 UDP association exists, or creates a new one
pub async fn ensure_socks5_udp_association(
    proxy: &Proxy,
    inbound_tag: &str,
    association: &Socks5UdpAssociation,
    session_id: u64,
    upstream_association: &mut Option<ActiveUpstreamSocks5UdpAssociation>,
    upstream_idle_deadline: &mut Option<TokioInstant>,
) -> Result<(), EngineError> {
    let needs_new_association = upstream_association
        .as_ref()
        .map(|a| !a.matches(&association.tag, &association.server, association.port))
        .unwrap_or(true);

    if !needs_new_association {
        proxy.record_udp_upstream_association_reused();
        log_udp_upstream_association_reused(
            inbound_tag,
            &association.tag,
            &association.server,
            association.port,
        );
        return Ok(());
    }

    if let Some(a) = upstream_association.take() {
        a.close(UpstreamAssociationCloseReason::Closed);
        *upstream_idle_deadline = None;
    }

    match ActiveUpstreamSocks5UdpAssociation::establish(
        proxy,
        &association.tag,
        &association.server,
        association.port,
        association
            .auth
            .as_ref()
            .map(|(u, p)| (u.as_str(), p.as_str())),
        session_id,
    )
    .await
    {
        Ok(a) => {
            proxy.record_udp_upstream_association_created();
            *upstream_idle_deadline = Some(TokioInstant::now() + proxy.udp_upstream_idle_timeout());
            log_udp_upstream_association_created(
                inbound_tag,
                &association.tag,
                &association.server,
                association.port,
                proxy.udp_upstream_idle_timeout(),
            );
            *upstream_association = Some(a);
            Ok(())
        }
        Err(error) => {
            proxy.record_udp_upstream_association_failed();
            Err(error)
        }
    }
}

/// Send a UDP packet through SOCKS5 upstream association
pub async fn send_socks5_udp_packet(
    proxy: &Proxy,
    inbound_tag: &str,
    association: &Socks5UdpAssociation,
    session: &Session,
    payload: &[u8],
    upstream_association: &mut Option<ActiveUpstreamSocks5UdpAssociation>,
    upstream_idle_deadline: &mut Option<TokioInstant>,
) -> Result<usize, EngineError> {
    ensure_socks5_udp_association(
        proxy,
        inbound_tag,
        association,
        session.id,
        upstream_association,
        upstream_idle_deadline,
    )
    .await?;

    let association_ref = upstream_association
        .as_ref()
        .expect("successful establish stores upstream association");

    match association_ref
        .send_packet(&session.target, session.port, payload)
        .await
    {
        Ok(sent) => {
            proxy.record_udp_upstream_packet_sent();
            *upstream_idle_deadline = Some(TokioInstant::now() + proxy.udp_upstream_idle_timeout());
            Ok(sent)
        }
        Err(error) => {
            proxy.record_udp_upstream_send_failure();
            if let Some(a) = upstream_association.take() {
                let outbound_tag = a.outbound_tag().to_owned();
                a.close(UpstreamAssociationCloseReason::Dropped);
                log_udp_upstream_association_dropped(
                    inbound_tag,
                    &outbound_tag,
                    &association.server,
                    association.port,
                    &error,
                );
            }
            *upstream_idle_deadline = None;
            Err(error)
        }
    }
}
