mod errors;
mod named;
mod runtime;
mod transport_bridge;

use std::path::Path;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_transport::managed_udp::{
    ProtocolManagedStreamUdpBridgeHandlerMetadata, ProtocolManagedStreamUdpBridgeOps,
    ProtocolRelayTwoStreamManagedUdpBridgeOps,
};
use zero_transport::outbound_leaf::{
    ProtocolRelayTwoStreamTransportLeaf, ProtocolRelayTwoStreamUdpTransportBridgeMetadata,
    ProtocolTcpTransportBridgeMetadata, ProtocolTcpTransportBridgeOps,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf, ProtocolTransportLeafResolver,
    ProtocolUdpTransportBridgeMetadata,
};

pub(crate) use named::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, transport_bridge_adapter_claims_runtime_leaf,
    NamedProtocolAdapter, ProtocolTransportBridgeAdapter,
};
pub(crate) use runtime::{
    direct_leaf_runtime, proxy_leaf_runtime, transport_bridge_adapter_leaf_runtime,
    unreachable_leaf, unreachable_udp_leaf,
};

use crate::protocol_registry::{OutboundAdapterContext, UdpAdapterContext};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::{ManagedStreamFlowConnector, ManagedStreamFlowHandler};
use crate::transport::{
    EstablishedTcpOutbound, RelayCarrier, StreamTraffic, TcpOutboundFailure, TcpRelayStream,
};

// Shared transport-bridge glue owns the neutral
// `EstablishedTcpOutbound::proxied` mapping plus generic `TcpOutboundFailure`
// / `FlowFailure` projection for typed transport leaves.

#[allow(dead_code)]
pub(super) fn tcp_transport_leaf<'a, T, E, F>(
    leaf: &ResolvedLeafOutbound<'a>,
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    invalid_leaf_stage: &'static str,
    expected_leaf: &'static str,
    build: F,
) -> Result<T, TcpOutboundFailure>
where
    E: std::fmt::Display,
    F: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<T>, E>,
{
    transport_bridge::tcp_transport_leaf(
        leaf,
        source_dir,
        stage,
        invalid_config,
        invalid_leaf_stage,
        expected_leaf,
        build,
    )
}

#[allow(dead_code)]
pub(super) fn tcp_relay_transport_leaf<'a, T, E, F>(
    leaf: &ResolvedLeafOutbound<'a>,
    source_dir: Option<&Path>,
    invalid_config: &'static str,
    invalid_leaf_stage: &'static str,
    expected_leaf: &'static str,
    build: F,
) -> Result<T, EngineError>
where
    E: std::fmt::Display,
    F: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<T>, E>,
{
    transport_bridge::tcp_relay_transport_leaf(
        leaf,
        source_dir,
        invalid_config,
        invalid_leaf_stage,
        expected_leaf,
        build,
    )
}

#[allow(dead_code)]
pub(super) fn udp_transport_leaf<'a, T, E, F>(
    leaf: &ResolvedLeafOutbound<'a>,
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
    build: F,
) -> Result<T, FlowFailure>
where
    E: std::fmt::Display,
    F: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<T>, E>,
{
    transport_bridge::udp_transport_leaf(
        leaf,
        source_dir,
        stage,
        invalid_config,
        expected_leaf,
        build,
    )
}

pub(crate) fn transport_bridge_adapter_managed_stream_udp_handler<A>(
) -> Box<dyn ManagedStreamFlowHandler>
where
    A: ProtocolTransportBridgeAdapter,
    A::Bridge: ProtocolManagedStreamUdpBridgeHandlerMetadata,
    <A::Bridge as ProtocolManagedStreamUdpBridgeHandlerMetadata>::Resume:
        ManagedStreamFlowConnector,
{
    transport_bridge::managed_stream_udp_handler_for_bridge::<A::Bridge>()
}

pub(crate) async fn connect_protocol_transport_bridge_adapter_tcp<'a, A, FInspect>(
    adapter: &A,
    ctx: OutboundAdapterContext<'_>,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'a>,
    inspect_traffic: FInspect,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>
where
    A: ProtocolTransportBridgeAdapter,
    A::Bridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<
            <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: ProtocolTransportLeaf,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
    <A::Bridge as ProtocolTcpTransportBridgeOps<
        <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
    >>::Opened: ProtocolTcpTransportOpenResult,
    FInspect: FnOnce(&StreamTraffic),
{
    transport_bridge::connect_protocol_transport_bridge_adapter_tcp(
        adapter,
        ctx,
        session,
        leaf,
        inspect_traffic,
    )
    .await
}

pub(crate) async fn apply_protocol_transport_bridge_adapter_relay_hop<'a, A>(
    adapter: &A,
    ctx: OutboundAdapterContext<'_>,
    stream: TcpRelayStream,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'a>,
) -> Result<TcpRelayStream, EngineError>
where
    A: ProtocolTransportBridgeAdapter,
    A::Bridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<
            <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: Sync,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    transport_bridge::apply_protocol_transport_bridge_adapter_relay_hop(
        adapter, ctx, stream, session, leaf,
    )
    .await
}

pub(crate) async fn start_protocol_transport_bridge_adapter_udp_flow<'a, A>(
    adapter: &A,
    dispatch: &mut UdpDispatch,
    ctx: UdpAdapterContext<'_>,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'a>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    A: ProtocolTransportBridgeAdapter,
    A::Bridge: ProtocolUdpTransportBridgeMetadata
        + ProtocolTransportLeafResolver<'a>
        + ProtocolManagedStreamUdpBridgeOps<
            <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: ProtocolTransportLeaf,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    transport_bridge::start_protocol_transport_bridge_adapter_udp_flow(
        adapter, dispatch, ctx, session, leaf, payload,
    )
    .await
}

pub(crate) async fn start_protocol_transport_bridge_adapter_udp_relay_final_hop<'a, A>(
    adapter: &A,
    dispatch: &mut UdpDispatch,
    ctx: UdpAdapterContext<'_>,
    session: &Session,
    carrier: RelayCarrier,
    leaf: &ResolvedLeafOutbound<'a>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    A: ProtocolTransportBridgeAdapter,
    A::Bridge: ProtocolUdpTransportBridgeMetadata
        + ProtocolTransportLeafResolver<'a>
        + ProtocolManagedStreamUdpBridgeOps<
            <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: ProtocolTransportLeaf,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    transport_bridge::start_protocol_transport_bridge_adapter_udp_relay_final_hop(
        adapter, dispatch, ctx, session, carrier, leaf, payload,
    )
    .await
}

pub(crate) fn protocol_transport_bridge_adapter_udp_relay_needs_two_streams<'a, A>(
    adapter: &A,
    leaf: &ResolvedLeafOutbound<'a>,
) -> bool
where
    A: ProtocolTransportBridgeAdapter,
    A::Bridge: ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + ProtocolTransportLeafResolver<'a>
        + ProtocolRelayTwoStreamManagedUdpBridgeOps<
            <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf:
        ProtocolRelayTwoStreamTransportLeaf,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    transport_bridge::protocol_transport_bridge_adapter_udp_relay_needs_two_streams(adapter, leaf)
}

pub(crate) async fn start_protocol_transport_bridge_adapter_udp_relay_two_stream<'a, 'chain, A>(
    adapter: &A,
    dispatch: &mut UdpDispatch,
    ctx: UdpAdapterContext<'_>,
    session: &Session,
    chain: &'chain [ResolvedLeafOutbound<'a>],
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    A: ProtocolTransportBridgeAdapter,
    A::Bridge: ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + ProtocolTransportLeafResolver<'a>
        + ProtocolRelayTwoStreamManagedUdpBridgeOps<
            <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf:
        ProtocolRelayTwoStreamTransportLeaf,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    transport_bridge::start_protocol_transport_bridge_adapter_udp_relay_two_stream(
        adapter, dispatch, ctx, session, chain, payload,
    )
    .await
}
