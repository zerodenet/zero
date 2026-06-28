use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor_from_build, PacketPathCarrier, PacketPathCarrierDescriptor,
};

pub(super) fn carrier_descriptor(
    leaf: &ResolvedLeafOutbound<'_>,
) -> Option<PacketPathCarrierDescriptor> {
    let ResolvedLeafOutbound::Hysteria2 {
        tag,
        server,
        port,
        password,
        client_fingerprint,
        ..
    } = leaf
    else {
        return None;
    };
    let descriptor = hysteria2::udp::udp_packet_path_carrier_descriptor_from_config(
        tag,
        server,
        *port,
        password,
        *client_fingerprint,
    );
    Some(packet_path_carrier_descriptor_from_build(descriptor))
}

pub(super) async fn build(
    adapter: &Hysteria2Adapter,
    leaf: &ResolvedLeafOutbound<'_>,
) -> Result<std::sync::Arc<dyn PacketPathCarrier>, EngineError> {
    let ResolvedLeafOutbound::Hysteria2 {
        server,
        port,
        password,
        client_fingerprint,
        ..
    } = leaf
    else {
        return Err(unreachable_leaf(adapter.name(), leaf).error);
    };
    let build = hysteria2::udp::udp_packet_path_carrier_build_from_config(
        "",
        server,
        *port,
        password,
        *client_fingerprint,
    );
    let (conn, codec) = crate::outbound::hysteria2::open_udp_packet_path_build(build).await?;
    let conn = std::sync::Arc::new(conn);
    crate::runtime::udp_flow::packet_path_chain::carriers::quic_datagram_carrier::build(conn, codec)
        .await
}
