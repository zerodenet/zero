use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;
use zero_transport::managed_udp::ProtocolManagedStreamUdpBridgeOps;
use zero_transport::outbound_leaf::{
    prepare_transport_bridge_leaf, prepared_relay_final_hop_udp_resume, ProtocolTransportLeaf,
    ProtocolTransportLeafResolver, ProtocolUdpTransportBridgeMetadata,
};

use super::super::error::{udp_flow_failure, udp_prepare_failure};
use super::super::stream_packet::{
    start_relay_managed_stream_packet, ManagedStreamPacketRelay, ManagedStreamPacketStartBridge,
};
use crate::runtime::udp_flow::result::{FlowFailure, FlowStartResult};
use crate::runtime::udp_flow::state::UdpFlowStartContext;
use crate::runtime::Proxy;
use crate::transport::RelayCarrier;

pub(crate) async fn start_protocol_transport_bridge_udp_relay_final_hop<'a, TBridge>(
    bridge: &TBridge,
    mut context: UdpFlowStartContext<'_>,
    proxy: &Proxy,
    session: &Session,
    carrier: RelayCarrier,
    leaf: &ResolvedLeafOutbound<'a>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    TBridge: ProtocolUdpTransportBridgeMetadata
        + ProtocolTransportLeafResolver<'a>
        + ProtocolManagedStreamUdpBridgeOps<
            <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: ProtocolTransportLeaf,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    let prepared = prepare_transport_bridge_leaf(bridge, proxy.config.source_dir(), leaf).map_err(
        |error| {
            udp_prepare_failure(
                leaf,
                error,
                TBridge::UDP_RELAY_FINAL_STAGE,
                TBridge::UDP_INVALID_CONFIG,
                TBridge::EXPECTED_OUTBOUND_LEAF,
            )
        },
    )?;
    let endpoint = prepared.endpoint();
    prepared.validate_udp_relay_final_hop().map_err(|error| {
        udp_flow_failure(
            "udp_relay_final_transport",
            error,
            Some((endpoint.server, endpoint.port)),
        )
    })?;
    start_relay_managed_stream_packet(
        &mut context,
        ManagedStreamPacketStartBridge::relay(
            Some(proxy),
            endpoint.tag,
            session,
            ManagedStreamPacketRelay {
                carrier,
                tls_server_name: None,
            },
            (endpoint.server, endpoint.port),
            prepared_relay_final_hop_udp_resume(bridge, &prepared),
            payload,
        ),
    )
    .await
}
