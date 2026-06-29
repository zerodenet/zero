use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
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
        self.into_parts()
    }
}

pub(super) fn carrier_descriptor(
    leaf: &ResolvedLeafOutbound<'_>,
) -> Option<PacketPathCarrierDescriptor> {
    let ResolvedLeafOutbound::Shadowsocks {
        tag,
        server,
        port,
        password,
        cipher,
    } = leaf
    else {
        return None;
    };
    let descriptor = shadowsocks::udp::udp_packet_path_carrier_descriptor_from_config(
        tag, server, *port, cipher, password,
    )
    .ok()?;
    Some(packet_path_carrier_descriptor_from_build(descriptor))
}

pub(super) async fn build(
    adapter: &ShadowsocksAdapter,
    proxy: &Proxy,
    leaf: &ResolvedLeafOutbound<'_>,
) -> Result<std::sync::Arc<dyn PacketPathCarrier>, EngineError> {
    let ResolvedLeafOutbound::Shadowsocks {
        server,
        port,
        password,
        cipher,
        ..
    } = leaf
    else {
        return Err(unreachable_leaf(adapter.name(), leaf).error);
    };
    let codec = shadowsocks::udp::udp_packet_path_carrier_codec_from_config(
        "", server, *port, cipher, password,
    )
    .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
    crate::runtime::udp_flow::packet_path_chain::carriers::udp_socket_carrier::build(
        proxy, server, *port, codec,
    )
    .await
}

pub(super) fn datagram_source(leaf: &ResolvedLeafOutbound<'_>) -> Option<UdpDatagramSource> {
    let ResolvedLeafOutbound::Shadowsocks {
        tag,
        server,
        port,
        password,
        cipher,
    } = leaf
    else {
        return None;
    };
    let datagram = shadowsocks::udp::udp_packet_path_datagram_source_build_from_config(
        tag, server, *port, cipher, password,
    )
    .ok()?;
    Some(udp_datagram_source_from_build(datagram))
}
