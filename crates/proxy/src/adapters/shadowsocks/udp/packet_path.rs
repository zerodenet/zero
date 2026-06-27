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
    let config = packet_path_config(tag, server, *port, cipher, password);
    let spec = config.packet_path_spec().ok()?;
    Some(packet_path_carrier_descriptor(
        spec.cache_key(),
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
    let config = packet_path_config("", server, *port, cipher, password);
    let spec = config
        .packet_path_spec()
        .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
    let codec = Arc::new(spec.codec());
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
    let config = packet_path_config(tag, server, *port, cipher, password);
    let spec = config.packet_path_spec().ok()?;
    let codec = Arc::new(spec.codec());
    Some(udp_datagram_source(
        tag,
        server,
        *port,
        spec.cache_key(),
        codec,
    ))
}

fn packet_path_config<'a>(
    tag: &'a str,
    server: &'a str,
    port: u16,
    cipher: &'a str,
    password: &'a str,
) -> shadowsocks::ShadowsocksUdpFlowConfig<'a> {
    shadowsocks::ShadowsocksUdpFlowConfig::new(tag, server, port, cipher, password)
}
