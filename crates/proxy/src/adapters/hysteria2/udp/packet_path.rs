use std::sync::Arc;

use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::unreachable_leaf;
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_flow::packet_path::{PacketPathCarrier, PacketPathCarrierDescriptor};

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
    let config = packet_path_config(tag, server, *port, password, *client_fingerprint);
    Some(
        crate::runtime::udp_flow::packet_path::packet_path_carrier_descriptor(
            config.packet_path_cache_key(),
            server,
            *port,
        ),
    )
}

pub(super) async fn build(
    adapter: &Hysteria2Adapter,
    leaf: &ResolvedLeafOutbound<'_>,
) -> Result<Arc<dyn PacketPathCarrier>, EngineError> {
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
    let config = packet_path_config("", server, *port, password, *client_fingerprint);
    let codec = Arc::new(config.packet_path_codec());
    let conn = Arc::new(
        crate::outbound::hysteria2::open_udp_packet_path_connection(
            server,
            *port,
            config.connector_profile(),
        )
        .await?,
    );
    crate::runtime::udp_flow::packet_path_chain::carriers::quic_datagram_carrier::build(conn, codec)
        .await
}

fn packet_path_config<'a>(
    tag: &'a str,
    server: &'a str,
    port: u16,
    password: &'a str,
    client_fingerprint: Option<&'a str>,
) -> hysteria2::Hysteria2UdpFlowConfig<'a> {
    hysteria2::Hysteria2UdpFlowConfig::new(tag, server, port, password, client_fingerprint)
}
