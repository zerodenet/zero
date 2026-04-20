use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use zero_core::{Address, Error as CoreError};
use zero_platform_tokio::{TokioDatagramSocket, TokioSocket};

use super::error::EngineError;
use super::runtime::Engine;
use super::stats::EngineStats;

pub(crate) struct ActiveUpstreamSocks5UdpAssociation {
    tag: String,
    server: String,
    port: u16,
    stats: Arc<EngineStats>,
    close_recorded: AtomicBool,
    _control: TokioSocket,
    relay: TokioDatagramSocket,
    relay_addr: SocketAddr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpstreamAssociationCloseReason {
    Closed,
    IdleTimeout,
    Dropped,
}

impl ActiveUpstreamSocks5UdpAssociation {
    pub(crate) async fn establish(
        engine: &Engine,
        tag: &str,
        server: &str,
        port: u16,
    ) -> Result<Self, EngineError> {
        let mut control = engine
            .protocols
            .direct_outbound
            .connect_host(server, port, &engine.resolver)
            .await?;
        let (relay_address, relay_port) = engine
            .protocols
            .socks5_outbound
            .establish_udp_association(&mut control)
            .await?;
        let relay_addr = engine
            .protocols
            .direct_outbound
            .resolve_address(
                &relay_address,
                relay_port,
                &engine.resolver,
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
            stats: Arc::clone(&engine.stats),
            close_recorded: AtomicBool::new(false),
            _control: control,
            relay,
            relay_addr,
        })
    }

    pub(crate) fn matches(&self, tag: &str, server: &str, port: u16) -> bool {
        self.tag == tag && self.server == server && self.port == port
    }

    pub(crate) fn outbound_tag(&self) -> &str {
        &self.tag
    }

    pub(crate) fn upstream_endpoint(&self) -> (&str, u16) {
        (&self.server, self.port)
    }

    pub(crate) fn close(self, reason: UpstreamAssociationCloseReason) {
        self.close_recorded.store(true, Ordering::Relaxed);

        match reason {
            UpstreamAssociationCloseReason::Closed => {
                self.stats.record_udp_upstream_association_closed();
            }
            UpstreamAssociationCloseReason::IdleTimeout => {
                self.stats.record_udp_upstream_association_idle_timeout();
            }
            UpstreamAssociationCloseReason::Dropped => {
                self.stats.record_udp_upstream_association_dropped();
            }
        }
    }

    pub(crate) async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let packet = zero_protocol_socks5::build_udp_packet(target, port, payload)?;
        self.relay.send_to_addr(&packet, self.relay_addr).await?;
        Ok(())
    }

    pub(crate) async fn recv_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        let (read, sender) = self.relay.recv_from_addr(buf).await?;
        if sender != self.relay_addr {
            return Err(CoreError::Protocol("unexpected UDP sender from SOCKS5 upstream").into());
        }

        Ok(read)
    }
}

impl Drop for ActiveUpstreamSocks5UdpAssociation {
    fn drop(&mut self) {
        if !self.close_recorded.load(Ordering::Relaxed) {
            self.stats.record_udp_upstream_association_closed();
            self.close_recorded.store(true, Ordering::Relaxed);
        }
    }
}
