use std::sync::atomic::{AtomicBool, Ordering};

use socks5::udp::Socks5EstablishedUdpAssociation;
use zero_core::Address;
use zero_engine::EngineError;
use zero_platform_tokio::{TokioDatagramSocket, TokioSocket};

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
        let server = target.server().to_owned();
        let port = target.port();
        let control = proxy
            .protocols
            .direct_connector()
            .connect_host(&server, port, proxy.resolver.as_ref())
            .await?;
        let mut control = MeteredStream::new(control);
        let (relay_address, relay_port) = target.establish_with_control(&mut control).await?;
        proxy.record_session_outbound_traffic(session_id, control.drain_traffic());
        let control = control.into_inner();
        let (relay_addr, relay) = crate::runtime::udp_helpers::resolve_udp_peer_endpoint(
            proxy,
            &relay_address,
            relay_port,
            "failed to resolve upstream socks5 udp relay",
        )
        .await?;

        Ok(Self {
            proxy: proxy.clone(),
            close_recorded: AtomicBool::new(false),
            association: Socks5EstablishedUdpAssociation::from_relay_socket_address(
                control,
                relay,
                zero_platform_tokio::socket_addr_to_socket_address(relay_addr),
            ),
        })
    }

    pub(super) fn close(
        self,
        reason: crate::runtime::udp_flow::registered::UpstreamAssociationCloseReason,
    ) {
        self.close_recorded.store(true, Ordering::Relaxed);

        match reason {
            crate::runtime::udp_flow::registered::UpstreamAssociationCloseReason::Closed => {
                self.proxy.record_udp_upstream_association_closed();
            }
            crate::runtime::udp_flow::registered::UpstreamAssociationCloseReason::IdleTimeout => {
                self.proxy.record_udp_upstream_association_idle_timeout();
            }
            crate::runtime::udp_flow::registered::UpstreamAssociationCloseReason::Dropped => {
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
impl crate::runtime::udp_flow::packet_path::PacketPathPayloadTransport
    for ActiveUpstreamSocks5UdpAssociation
{
    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.send_packet(target, port, payload).await?;
        Ok(())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        ActiveUpstreamSocks5UdpAssociation::recv_payload(self, buf).await
    }
}

#[async_trait::async_trait]
impl
    crate::runtime::udp_flow::registered::UpstreamAssociationTransport<
        socks5::udp::Socks5UdpAssociationTarget,
    > for ActiveUpstreamSocks5UdpAssociation
{
    async fn establish(
        proxy: &Proxy,
        target: socks5::udp::Socks5UdpAssociationTarget,
        session_id: u64,
    ) -> Result<Self, EngineError> {
        Self::establish(proxy, target, session_id).await
    }

    async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        Self::send_packet(self, target, port, payload).await
    }

    async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), EngineError> {
        Self::recv_response_parts(self, buf).await
    }

    fn close(self, reason: crate::runtime::udp_flow::registered::UpstreamAssociationCloseReason) {
        Self::close(self, reason);
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
