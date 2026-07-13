use crate::protocol_registry::{prepare_transport_bridge_leaf, ProtocolTransportLeafResolver};
use zero_engine::ResolvedLeafOutbound;
use zero_transport::managed_udp::ProtocolRelayTwoStreamManagedUdpBridgeOps;
use zero_transport::outbound_leaf::{
    prepared_udp_relay_needs_two_streams, ProtocolRelayTwoStreamTransportLeaf,
    ProtocolRelayTwoStreamUdpTransportBridgeMetadata,
};

pub(crate) fn protocol_transport_bridge_udp_relay_needs_two_streams<'a, TBridge>(
    bridge: &TBridge,
    leaf: &ResolvedLeafOutbound<'a>,
) -> bool
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
    prepare_transport_bridge_leaf(bridge, None, leaf)
        .is_ok_and(|prepared| prepared_udp_relay_needs_two_streams(bridge, &prepared))
}
