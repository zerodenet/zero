use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor, udp_datagram_source_from_build, PacketPathCarrier,
    PacketPathCarrierDescriptor, UdpDatagramSource,
};
use crate::runtime::Proxy;

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
    let spec =
        shadowsocks::udp_packet_path_spec_from_config(tag, server, *port, cipher, password).ok()?;
    let descriptor = spec.carrier_descriptor(server, *port);
    Some(packet_path_carrier_descriptor(
        descriptor.cache_key(),
        descriptor.server(),
        descriptor.port(),
    ))
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
    let spec = shadowsocks::udp_packet_path_spec_from_config("", server, *port, cipher, password)
        .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
    let datagram = spec.datagram_source_build("", server, *port);
    crate::runtime::udp_flow::packet_path_chain::carriers::udp_socket_carrier::build(
        proxy,
        server,
        *port,
        udp_datagram_source_from_build(datagram).into_codec(),
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
    let spec =
        shadowsocks::udp_packet_path_spec_from_config(tag, server, *port, cipher, password).ok()?;
    let datagram = spec.datagram_source_build(tag, server, *port);
    Some(udp_datagram_source_from_build(datagram))
}
