use std::sync::Arc;

use async_trait::async_trait;
use zero_core::Address;
use zero_engine::EngineError;

use super::active::ActiveUpstreamSocks5UdpAssociation;
use crate::runtime::Proxy;

pub(crate) struct Socks5PacketPath {
    association: Arc<ActiveUpstreamSocks5UdpAssociation>,
}

#[async_trait]
impl crate::runtime::udp_flow::packet_path::PacketPathCarrier for Socks5PacketPath {
    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.association.send_packet(target, port, payload).await?;
        Ok(())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.association.recv_payload(buf).await
    }
}

pub(crate) async fn build_socks5_packet_path(
    proxy: &Proxy,
    tag: &str,
    server: &str,
    port: u16,
    packet_path: socks5::Socks5UdpPacketPath<'_>,
) -> Result<Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>, EngineError> {
    let association = Arc::new(
        ActiveUpstreamSocks5UdpAssociation::establish(
            proxy,
            tag,
            server,
            port,
            packet_path.association_config(),
            0,
        )
        .await?,
    );
    Ok(Arc::new(Socks5PacketPath { association }))
}
