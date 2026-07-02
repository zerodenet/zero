use zero_engine::EngineError;

use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor_from_build, udp_datagram_source_from_build, PacketPathCarrier,
    PacketPathCarrierDescriptor, UdpDatagramSource,
};
use crate::runtime::Proxy;

impl crate::runtime::udp_flow::packet_path::UdpDatagramSourceBuild
    for shadowsocks::udp::ShadowsocksUdpPacketPathDatagramSourceBuild
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
        self.into_shared_codec_parts()
    }
}

impl crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptorBuild
    for shadowsocks::udp::ShadowsocksUdpPacketPathCarrierDescriptor
{
    fn into_parts(self) -> (String, String, u16) {
        shadowsocks::udp::ShadowsocksUdpPacketPathCarrierDescriptor::into_parts(self)
    }
}

pub(super) fn carrier_descriptor(
    descriptor: shadowsocks::udp::ShadowsocksUdpPacketPathCarrierDescriptor,
) -> PacketPathCarrierDescriptor {
    packet_path_carrier_descriptor_from_build(descriptor)
}

pub(super) async fn build(
    proxy: &Proxy,
    server: &str,
    port: u16,
    codec: std::sync::Arc<
        dyn zero_traits::DatagramCodec<zero_core::Address, Error = zero_core::Error>,
    >,
) -> Result<std::sync::Arc<dyn PacketPathCarrier>, EngineError> {
    crate::runtime::udp_flow::packet_path_chain::carriers::udp_socket_carrier::build(
        proxy, server, port, codec,
    )
    .await
}

pub(super) fn datagram_source(
    datagram: shadowsocks::udp::ShadowsocksUdpPacketPathDatagramSourceBuild,
) -> UdpDatagramSource {
    udp_datagram_source_from_build(datagram)
}
