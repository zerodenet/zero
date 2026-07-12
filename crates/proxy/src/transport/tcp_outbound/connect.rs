use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;
use zero_transport::outbound_leaf::{
    open_prepared_tcp_transport_bridge_stream, prepare_transport_bridge_leaf,
    ProtocolTcpTransportBridgeMetadata, ProtocolTcpTransportBridgeOps,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf, ProtocolTransportLeafResolver,
};
use zero_transport::StreamTraffic;

use super::error::{tcp_connect_prepare_failure, tcp_failure};
use super::model::{EstablishedTcpOutbound, TcpOutboundFailure};
use crate::protocol_registry::OutboundAdapterContext;

pub(crate) async fn connect_protocol_transport_bridge_tcp<'a, TBridge, FInspect>(
    bridge: &TBridge,
    ctx: OutboundAdapterContext<'_>,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'a>,
    inspect_traffic: FInspect,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>
where
    TBridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: ProtocolTransportLeaf,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
    <TBridge as ProtocolTcpTransportBridgeOps<
        <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
    >>::Opened: ProtocolTcpTransportOpenResult,
    FInspect: FnOnce(&StreamTraffic),
{
    let proxy = ctx.proxy();
    let prepared = prepare_transport_bridge_leaf(bridge, proxy.config.source_dir(), leaf).map_err(
        |error| {
            tcp_connect_prepare_failure(
                leaf,
                error,
                TBridge::TCP_CONNECT_STAGE,
                TBridge::TCP_INVALID_CONNECT_CONFIG,
                TBridge::TCP_INVALID_CONNECT_LEAF_STAGE,
                TBridge::EXPECTED_OUTBOUND_LEAF,
            )
        },
    )?;
    let endpoint = prepared.endpoint();
    let opened = open_prepared_tcp_transport_bridge_stream(
        bridge,
        session,
        &prepared,
        move |server, port| proxy.connect_upstream_host_owned(server.to_owned(), port),
    )
    .await
    .map_err(|error| {
        tcp_failure(
            TBridge::TCP_CONNECT_STAGE,
            error,
            Some((endpoint.server, endpoint.port)),
        )
    })?;
    let (stream, traffic) = opened.into_proxied_stream_parts();
    inspect_traffic(&traffic);
    if !traffic.is_empty() {
        proxy.record_session_outbound_traffic(session.id, traffic);
    }
    Ok(EstablishedTcpOutbound::proxied(
        endpoint.tag,
        endpoint.server,
        endpoint.port,
        stream,
    ))
}
