use zero_engine::EngineError;
use zero_transport::shadowsocks_transport::{
    ShadowsocksManagedUdpPacketPathCarrierDescriptor,
    ShadowsocksManagedUdpPacketPathDatagramSourceBuild, ShadowsocksManagedUdpPacketPathPlan,
};

use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor_from_build, udp_datagram_source_from_build, PacketPathCarrier,
    PacketPathCarrierDescriptor, UdpDatagramSource,
};
use crate::runtime::Proxy;

impl crate::runtime::udp_flow::packet_path::UdpDatagramSourceBuild
    for ShadowsocksManagedUdpPacketPathDatagramSourceBuild
{
    fn into_parts(
        self,
    ) -> (
        String,
        String,
        u16,
        String,
        std::sync::Arc<
            dyn zero_traits::DatagramCodec<zero_core::Address, Error = zero_core::Error>,
        >,
    ) {
        ShadowsocksManagedUdpPacketPathDatagramSourceBuild::into_shared_codec_parts(self)
    }
}

impl crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptorBuild
    for ShadowsocksManagedUdpPacketPathCarrierDescriptor
{
    fn into_parts(self) -> (String, String, u16) {
        ShadowsocksManagedUdpPacketPathCarrierDescriptor::into_parts(self)
    }
}

pub(super) fn carrier_descriptor(
    plan: ShadowsocksManagedUdpPacketPathPlan<'_>,
) -> PacketPathCarrierDescriptor {
    packet_path_carrier_descriptor_from_build(plan.into_carrier_descriptor())
}

pub(super) async fn build(
    proxy: &Proxy,
    plan: ShadowsocksManagedUdpPacketPathPlan<'_>,
) -> Result<std::sync::Arc<dyn PacketPathCarrier>, EngineError> {
    crate::runtime::udp_flow::packet_path_chain::carriers::udp_socket_carrier::build(
        proxy,
        plan.server(),
        plan.port(),
        plan.carrier_codec(),
    )
    .await
}

pub(super) fn datagram_source(plan: ShadowsocksManagedUdpPacketPathPlan<'_>) -> UdpDatagramSource {
    udp_datagram_source_from_build(plan.into_datagram_source_build())
}
