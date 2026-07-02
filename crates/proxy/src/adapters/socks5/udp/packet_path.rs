use zero_engine::EngineError;

use super::active::ActiveUpstreamSocks5UdpAssociation;
use crate::runtime::Proxy;

impl crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptorBuild
    for socks5::udp::Socks5UdpPacketPathCarrierDescriptor
{
    fn into_parts(self) -> (String, String, u16) {
        socks5::udp::Socks5UdpPacketPathCarrierDescriptor::into_parts(self)
    }
}

pub(super) fn carrier_descriptor(
    descriptor: socks5::udp::Socks5UdpPacketPathCarrierDescriptor,
) -> crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor {
    crate::runtime::udp_flow::packet_path::packet_path_carrier_descriptor_from_build(descriptor)
}

pub(super) async fn build(
    proxy: &Proxy,
    carrier: socks5::udp::Socks5UdpPacketPathCarrierBuild,
) -> Result<std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>, EngineError>
{
    let target = socks5::udp::packet_path_carrier_association_target(carrier);
    let association =
        std::sync::Arc::new(ActiveUpstreamSocks5UdpAssociation::establish(proxy, target, 0).await?);
    Ok(crate::runtime::udp_flow::packet_path::packet_path_payload_carrier(association))
}
