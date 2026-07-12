use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;
use zero_transport::managed_udp::ProtocolRelayTwoStreamManagedUdpBridgeOps;
use zero_transport::outbound_leaf::{
    open_prepared_relay_two_stream_udp_transport, prepare_last_transport_bridge_leaf,
    prepared_relay_two_stream_udp_resume, ProtocolRelayTwoStreamTransportLeaf,
    ProtocolRelayTwoStreamUdpTransportBridgeMetadata, ProtocolTransportLeafResolver,
};

use super::super::super::error::last_udp_prepare_failure;
use super::flow::start_relay_two_stream_managed_flow;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;

pub(crate) async fn start_protocol_transport_bridge_udp_relay_two_stream<'a, 'chain, TBridge>(
    bridge: &TBridge,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    chain: &'chain [ResolvedLeafOutbound<'a>],
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    TBridge: ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + ProtocolTransportLeafResolver<'a>
        + ProtocolRelayTwoStreamManagedUdpBridgeOps<
            <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf:
        ProtocolRelayTwoStreamTransportLeaf,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    let prepared = prepare_last_transport_bridge_leaf(bridge, chain, proxy.config.source_dir())
        .map_err(|error| {
            last_udp_prepare_failure(
                chain,
                error,
                TBridge::UDP_RELAY_CAPABILITY_STAGE,
                TBridge::UDP_INVALID_CONFIG,
                TBridge::EXPECTED_OUTBOUND_LEAF,
            )
        })?;
    let endpoint = prepared.endpoint();
    let resume = prepared_relay_two_stream_udp_resume(bridge, &prepared);
    start_relay_two_stream_managed_flow(
        dispatch,
        proxy,
        session,
        chain,
        endpoint.tag,
        endpoint.server,
        endpoint.port,
        TBridge::UDP_RELAY_CHAIN_STAGE,
        |post_stream, get_stream| {
            open_prepared_relay_two_stream_udp_transport(&prepared, post_stream, get_stream)
        },
        resume,
        payload,
    )
    .await
}
