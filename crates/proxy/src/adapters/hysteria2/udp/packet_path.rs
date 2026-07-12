use zero_engine::EngineError;
use zero_transport::hysteria2_quic::{
    Hysteria2ManagedUdpPacketPathCarrierDescriptor, Hysteria2ManagedUdpPacketPathPlan,
};

use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor_from_build, DatagramCodec, PacketPathCarrier,
    PacketPathCarrierDescriptor, PacketPathCarrierDescriptorBuild,
};

impl PacketPathCarrierDescriptorBuild for Hysteria2ManagedUdpPacketPathCarrierDescriptor {
    fn into_parts(self) -> (String, String, u16) {
        Hysteria2ManagedUdpPacketPathCarrierDescriptor::into_parts(self)
    }
}

pub(super) fn carrier_descriptor(
    plan: Hysteria2ManagedUdpPacketPathPlan,
) -> PacketPathCarrierDescriptor {
    packet_path_carrier_descriptor_from_build(plan.into_carrier_descriptor())
}

pub(super) async fn build(
    plan: Hysteria2ManagedUdpPacketPathPlan,
) -> Result<std::sync::Arc<dyn PacketPathCarrier>, EngineError> {
    let (conn, codec): (
        quinn::Connection,
        std::sync::Arc<dyn DatagramCodec<zero_core::Address, Error = zero_core::Error>>,
    ) = zero_transport::hysteria2_quic::open_hysteria2_udp_packet_path_build(
        plan.into_carrier_build(),
    )
    .await?;
    let conn = std::sync::Arc::new(conn);
    crate::runtime::udp_flow::packet_path_chain::carriers::quic_datagram_carrier::build(conn, codec)
        .await
}
