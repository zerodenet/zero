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
impl crate::protocol_runtime::udp::PacketPathCarrier for Socks5PacketPath {
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
        let read = self.association.recv_packet(buf).await?;
        let packet = socks5::decode_udp_associate_response(&buf[..read])
            .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
        let len = packet.payload.len();
        buf[..len].copy_from_slice(&packet.payload);
        Ok(len)
    }
}

pub(crate) async fn build_socks5_packet_path(
    proxy: &Proxy,
    tag: &str,
    server: &str,
    port: u16,
    auth: Option<(&str, &str)>,
) -> Result<Arc<dyn crate::protocol_runtime::udp::PacketPathCarrier>, EngineError> {
    let association = Arc::new(
        ActiveUpstreamSocks5UdpAssociation::establish(proxy, tag, server, port, auth, 0).await?,
    );
    Ok(Arc::new(Socks5PacketPath { association }))
}
