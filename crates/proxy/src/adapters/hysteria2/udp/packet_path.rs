use zero_engine::EngineError;

use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor_from_build, PacketPathCarrier, PacketPathCarrierDescriptor,
};

pub(super) fn carrier_descriptor(
    descriptor: hysteria2::udp::Hysteria2UdpPacketPathCarrierDescriptor,
) -> PacketPathCarrierDescriptor {
    packet_path_carrier_descriptor_from_build(descriptor)
}

pub(super) async fn build(
    build: hysteria2::udp::Hysteria2UdpPacketPathCarrierBuild,
) -> Result<std::sync::Arc<dyn PacketPathCarrier>, EngineError> {
    let (conn, codec) = super::connector::open_udp_packet_path_build(build).await?;
    let conn = std::sync::Arc::new(conn);
    crate::runtime::udp_flow::packet_path_chain::carriers::quic_datagram_carrier::build(conn, codec)
        .await
}
