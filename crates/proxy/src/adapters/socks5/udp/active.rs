use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};

use socks5::{Socks5UdpAssociation, Socks5UdpRelayError};
use zero_core::Address;
use zero_engine::EngineError;
use zero_platform_tokio::{TokioDatagramSocket, TokioSocket};

use super::model::UpstreamAssociationCloseReason;
use crate::runtime::Proxy;
use crate::transport::MeteredStream;

/// Active SOCKS5 UDP upstream association.
pub(super) struct ActiveUpstreamSocks5UdpAssociation {
    outbound_tag: String,
    server: String,
    port: u16,
    proxy: Proxy,
    close_recorded: AtomicBool,
    association: Socks5UdpAssociation<TokioSocket, TokioDatagramSocket>,
}

impl ActiveUpstreamSocks5UdpAssociation {
    pub(super) async fn establish(
        proxy: &Proxy,
        outbound_tag: &str,
        server: &str,
        port: u16,
        config: socks5::Socks5UdpAssociationConfig<'_>,
        session_id: u64,
    ) -> Result<Self, EngineError> {
        let control = proxy
            .protocols
            .direct_connector()
            .connect_host(server, port, proxy.resolver.as_ref())
            .await?;
        let mut control = MeteredStream::new(control);
        let relay_target = socks5::establish_udp_relay_with_control(&mut control, config).await?;
        proxy.record_session_outbound_traffic(session_id, control.drain_traffic());
        let control = control.into_inner();
        let relay_addr = proxy
            .protocols
            .direct_connector()
            .resolve_address(
                &relay_target.address,
                relay_target.port,
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
            outbound_tag: outbound_tag.to_owned(),
            server: server.to_owned(),
            port,
            proxy: proxy.clone(),
            close_recorded: AtomicBool::new(false),
            association: Socks5UdpAssociation::from_relay_endpoint(
                control,
                relay,
                zero_platform_tokio::socket_addr_to_ip(relay_addr),
                relay_addr.port(),
            ),
        })
    }

    pub(super) fn matches(&self, outbound_tag: &str, server: &str, port: u16) -> bool {
        self.outbound_tag == outbound_tag && self.server == server && self.port == port
    }

    pub(super) fn outbound_tag(&self) -> &str {
        &self.outbound_tag
    }

    pub(super) fn upstream_endpoint(&self) -> (&str, u16) {
        (&self.server, self.port)
    }

    pub(super) fn close(self, reason: UpstreamAssociationCloseReason) {
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

    pub(super) async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        match self.association.send_packet(target, port, payload).await {
            Ok(sent) => Ok(sent),
            Err(Socks5UdpRelayError::Socket(error)) => Err(error.into()),
            Err(Socks5UdpRelayError::Protocol(error)) => Err(error.into()),
        }
    }

    pub(super) async fn recv_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        match self.association.recv_packet(buf).await {
            Ok(read) => Ok(read),
            Err(Socks5UdpRelayError::Socket(error)) => Err(error.into()),
            Err(Socks5UdpRelayError::Protocol(error)) => Err(error.into()),
        }
    }

    pub(super) async fn recv_payload(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        match self.association.recv_payload(buf).await {
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
