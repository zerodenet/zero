use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, Ordering};

use socks5::udp::Socks5EstablishedUdpAssociation;
use zero_core::Address;
use zero_engine::EngineError;
use zero_platform_tokio::{TokioDatagramSocket, TokioSocket};

use super::model::{
    Socks5UdpAssociationHandle, Socks5UdpPacketPathAssociation, UpstreamAssociationCloseReason,
};
use crate::runtime::Proxy;
use crate::transport::MeteredStream;

/// Active SOCKS5 UDP upstream association.
pub(super) struct ActiveUpstreamSocks5UdpAssociation {
    proxy: Proxy,
    close_recorded: AtomicBool,
    association: Socks5EstablishedUdpAssociation<TokioSocket, TokioDatagramSocket>,
}

impl ActiveUpstreamSocks5UdpAssociation {
    pub(super) async fn establish(
        proxy: &Proxy,
        target: socks5::udp::Socks5UdpAssociationTarget,
        session_id: u64,
    ) -> Result<Self, EngineError> {
        let (server, port) = target.connect_endpoint().into_parts();
        let control = proxy
            .protocols
            .direct_connector()
            .connect_host(&server, port, proxy.resolver.as_ref())
            .await?;
        let mut control = MeteredStream::new(control);
        let relay_target = socks5::udp::establish_udp_relay_with_control(
            &mut control,
            target.association_config(),
        )
        .await?;
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
            proxy: proxy.clone(),
            close_recorded: AtomicBool::new(false),
            association: Socks5EstablishedUdpAssociation::from_relay_socket_address(
                target,
                control,
                relay,
                zero_platform_tokio::socket_addr_to_socket_address(relay_addr),
            ),
        })
    }

    pub(super) fn outbound_tag(&self) -> &str {
        self.association.outbound_tag()
    }

    pub(super) fn identity(&self) -> socks5::udp::Socks5UdpAssociationIdentity {
        self.association.identity()
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
        self.association
            .send_packet(target, port, payload)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }

    pub(super) async fn recv_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.association
            .recv_packet(buf)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }

    pub(super) async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), EngineError> {
        self.association
            .recv_response_parts(buf)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }

    pub(super) async fn recv_payload(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.association
            .recv_payload(buf)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }
}

#[async_trait::async_trait]
impl Socks5UdpAssociationHandle for ActiveUpstreamSocks5UdpAssociation {
    fn outbound_tag(&self) -> &str {
        self.outbound_tag()
    }

    fn identity(&self) -> socks5::udp::Socks5UdpAssociationIdentity {
        self.identity()
    }

    fn close(self: Box<Self>, reason: UpstreamAssociationCloseReason) {
        (*self).close(reason);
    }

    async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.send_packet(target, port, payload).await
    }

    async fn recv_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.recv_packet(buf).await
    }

    async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), EngineError> {
        self.recv_response_parts(buf).await
    }
}

#[async_trait::async_trait]
impl Socks5UdpPacketPathAssociation for ActiveUpstreamSocks5UdpAssociation {
    async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.send_packet(target, port, payload).await
    }

    async fn recv_payload(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.recv_payload(buf).await
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
