use std::sync::Arc;

use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor, udp_datagram_source, PacketPathCarrier,
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
    let packet_path = packet_path_config(tag, server, *port, cipher, password).ok()?;
    Some(packet_path_carrier_descriptor(
        packet_path.cache_key(),
        server,
        *port,
    ))
}

pub(super) async fn build(
    adapter: &ShadowsocksAdapter,
    proxy: &Proxy,
    leaf: &ResolvedLeafOutbound<'_>,
) -> Result<Arc<dyn PacketPathCarrier>, EngineError> {
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
    let packet_path = packet_path_config("", server, *port, cipher, password)
        .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
    let codec = Arc::new(packet_path.codec());
    crate::runtime::udp_flow::packet_path_chain::carriers::udp_socket_carrier::build(
        proxy, server, *port, codec,
    )
    .await
}

pub(super) fn datagram_source<'a>(
    leaf: &ResolvedLeafOutbound<'a>,
) -> Option<UdpDatagramSource<'a>> {
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
    let packet_path = packet_path_config(tag, server, *port, cipher, password).ok()?;
    let codec = Arc::new(packet_path.codec());
    Some(udp_datagram_source(
        tag,
        server,
        *port,
        packet_path.cache_key(),
        codec,
    ))
}

fn packet_path_config<'a>(
    tag: &'a str,
    server: &'a str,
    port: u16,
    cipher: &'a str,
    password: &'a str,
) -> Result<shadowsocks::ShadowsocksUdpPacketPath, zero_core::Error> {
    shadowsocks::ShadowsocksUdpFlowConfig::new(tag, server, port, cipher, password).packet_path()
}
