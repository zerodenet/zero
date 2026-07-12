use zero_core::Address;
use zero_engine::EngineError;
use zero_transport::socks5_transport::{
    Socks5ManagedUdpAssociationTarget, Socks5UpstreamAssociationCloseReason,
    Socks5UpstreamUdpAssociation,
};

use crate::runtime::Proxy;

#[derive(Clone)]
struct ProxySocks5UdpAssociationRuntime {
    proxy: Proxy,
}

impl ProxySocks5UdpAssociationRuntime {
    fn new(proxy: Proxy) -> Self {
        Self { proxy }
    }
}

#[async_trait::async_trait]
impl zero_transport::socks5_transport::Socks5UdpAssociationRuntime
    for ProxySocks5UdpAssociationRuntime
{
    async fn open_control_socket(
        &self,
        server: &str,
        port: u16,
    ) -> Result<zero_platform_tokio::TokioSocket, EngineError> {
        self.proxy
            .connect_upstream_host_owned(server.to_owned(), port)
            .await
    }

    async fn resolve_udp_relay(
        &self,
        relay_address: Address,
        relay_port: u16,
    ) -> Result<
        (
            zero_traits::SocketAddress,
            zero_platform_tokio::TokioDatagramSocket,
        ),
        EngineError,
    > {
        let (relay_addr, relay) = crate::runtime::udp_helpers::resolve_udp_peer_endpoint(
            &self.proxy,
            &relay_address,
            relay_port,
            "failed to resolve upstream socks5 udp relay",
        )
        .await?;
        Ok((
            zero_platform_tokio::socket_addr_to_socket_address(relay_addr),
            relay,
        ))
    }

    fn record_control_traffic(
        &self,
        session_id: u64,
        control: &mut crate::transport::MeteredStream<zero_platform_tokio::TokioSocket>,
    ) {
        self.proxy
            .record_session_outbound_traffic(session_id, control.drain_traffic());
    }

    fn record_close(&self, reason: Socks5UpstreamAssociationCloseReason) {
        match reason {
            Socks5UpstreamAssociationCloseReason::Closed => {
                self.proxy.record_udp_upstream_association_closed();
            }
            Socks5UpstreamAssociationCloseReason::IdleTimeout => {
                self.proxy.record_udp_upstream_association_idle_timeout();
            }
            Socks5UpstreamAssociationCloseReason::Dropped => {
                self.proxy.record_udp_upstream_association_dropped();
            }
        }
    }
}

pub(super) async fn establish_packet_path_association(
    proxy: &Proxy,
    build: zero_transport::socks5_transport::Socks5ManagedUdpPacketPathCarrierBuild,
) -> Result<Socks5UpstreamUdpAssociation, EngineError> {
    zero_transport::socks5_transport::establish_packet_path_udp_association(
        ProxySocks5UdpAssociationRuntime::new(proxy.clone()),
        build,
        0,
    )
    .await
}

#[async_trait::async_trait]
impl crate::runtime::udp_flow::packet_path::PacketPathPayloadTransport
    for Socks5UpstreamUdpAssociation
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
        self.recv_payload(buf).await
    }
}

#[async_trait::async_trait]
impl
    crate::runtime::udp_flow::registered::UpstreamAssociationTransport<
        Socks5ManagedUdpAssociationTarget,
    > for Socks5UpstreamUdpAssociation
{
    async fn establish(
        proxy: &Proxy,
        target: Socks5ManagedUdpAssociationTarget,
        session_id: u64,
    ) -> Result<Self, EngineError> {
        zero_transport::socks5_transport::establish_registered_udp_association(
            ProxySocks5UdpAssociationRuntime::new(proxy.clone()),
            target,
            session_id,
        )
        .await
    }

    async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.send_packet(target, port, payload).await
    }

    async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), EngineError> {
        self.recv_response_parts(buf).await
    }

    fn close(self, reason: crate::runtime::udp_flow::registered::UpstreamAssociationCloseReason) {
        let reason = match reason {
            crate::runtime::udp_flow::registered::UpstreamAssociationCloseReason::Closed => {
                Socks5UpstreamAssociationCloseReason::Closed
            }
            crate::runtime::udp_flow::registered::UpstreamAssociationCloseReason::IdleTimeout => {
                Socks5UpstreamAssociationCloseReason::IdleTimeout
            }
            crate::runtime::udp_flow::registered::UpstreamAssociationCloseReason::Dropped => {
                Socks5UpstreamAssociationCloseReason::Dropped
            }
        };
        self.close(reason);
    }
}
