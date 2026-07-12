use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_transport::outbound_leaf::{
    open_prepared_tcp_transport_bridge_relay_hop, prepare_transport_bridge_leaf,
    ProtocolTcpTransportBridgeMetadata, ProtocolTcpTransportBridgeOps,
    ProtocolTransportLeafResolver,
};

use super::error::tcp_relay_prepare_error;
use crate::protocol_registry::OutboundAdapterContext;
use crate::transport::TcpRelayStream;

pub(crate) async fn apply_protocol_transport_bridge_relay_hop<'a, TBridge>(
    bridge: &TBridge,
    ctx: OutboundAdapterContext<'_>,
    stream: TcpRelayStream,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'a>,
) -> Result<TcpRelayStream, EngineError>
where
    TBridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: Sync,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    let prepared = prepare_transport_bridge_leaf(bridge, ctx.proxy().config.source_dir(), leaf)
        .map_err(|error| {
            tcp_relay_prepare_error(
                error,
                TBridge::TCP_INVALID_RELAY_CONFIG,
                TBridge::TCP_INVALID_RELAY_LEAF_STAGE,
                TBridge::EXPECTED_OUTBOUND_LEAF,
            )
        })?;
    open_prepared_tcp_transport_bridge_relay_hop(bridge, stream, session, &prepared).await
}
