use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;
use zero_transport::managed_udp::ProtocolManagedStreamUdpBridgeOps;
use zero_transport::outbound_leaf::{
    prepare_transport_bridge_leaf, prepared_direct_udp_resume, ProtocolTransportLeaf,
    ProtocolTransportLeafResolver, ProtocolUdpTransportBridgeMetadata,
};

use super::super::error::udp_prepare_failure;
use super::super::stream_packet::{
    start_direct_managed_stream_packet, ManagedStreamPacketStartBridge,
};
use crate::runtime::udp_flow::result::{FlowFailure, FlowStartResult};
use crate::runtime::udp_flow::state::UdpFlowStartContext;
use crate::runtime::Proxy;

pub(crate) async fn start_protocol_transport_bridge_udp_flow<'a, TBridge>(
    bridge: &TBridge,
    mut context: UdpFlowStartContext<'_>,
    proxy: &Proxy,
    session: &Session,
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
                TBridge::UDP_DIRECT_STAGE,
                TBridge::UDP_INVALID_CONFIG,
                TBridge::EXPECTED_OUTBOUND_LEAF,
            )
        },
    )?;
    let endpoint = prepared.endpoint();
    start_direct_managed_stream_packet(
        &mut context,
        ManagedStreamPacketStartBridge::direct(
            proxy,
            endpoint.tag,
            session,
            (endpoint.server, endpoint.port),
            prepared_direct_udp_resume(bridge, &prepared),
            payload,
        ),
    )
    .await
}
