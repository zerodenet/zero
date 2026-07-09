use std::future::Future;
use std::path::Path;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_transport::managed_udp::{
    ProtocolManagedStreamUdpBridgeHandlerMetadata, ProtocolManagedStreamUdpBridgeOps,
    ProtocolRelayTwoStreamManagedUdpBridgeOps,
};
use zero_transport::outbound_leaf::{
    open_relay_two_stream_udp_transport, open_tcp_transport_bridge_relay_hop,
    open_tcp_transport_bridge_stream, resolve_last_transport_leaf, resolve_transport_leaf,
    transport_leaf_endpoint, ProtocolRelayTwoStreamTransportLeaf,
    ProtocolRelayTwoStreamUdpTransportBridgeMetadata, ProtocolTcpTransportBridgeMetadata,
    ProtocolTcpTransportBridgeOps, ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
    ProtocolTransportLeafResolver, ProtocolUdpTransportBridgeMetadata, ResolveTransportLeafError,
};

use super::errors::{
    expected_outbound_leaf_error, invalid_input_error, prefixed_expected_outbound_leaf_error,
    relay_chain_flow_failure, tcp_failure, udp_flow_failure,
};
use super::named::ProtocolTransportBridgeAdapter;
use crate::protocol_registry::{OutboundAdapterContext, UdpAdapterContext};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::{
    managed_stream_handler_box, start_direct_managed_stream_packet,
    start_relay_managed_stream_packet, ManagedStreamFlowConnector, ManagedStreamFlowHandler,
};
use crate::runtime::Proxy;
use crate::transport::{
    EstablishedTcpOutbound, RelayCarrier, StreamTraffic, TcpOutboundFailure, TcpRelayStream,
};

fn proxy_upstream_endpoint<'a>(leaf: &'a ResolvedLeafOutbound<'a>) -> Option<(&'a str, u16)> {
    leaf.proxy_endpoint()
}

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
    let upstream = proxy_upstream_endpoint(leaf);
    resolve_transport_leaf(source_dir, leaf, build).map_err(|error| match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            tcp_failure(stage, invalid_input_error(invalid_config, error), upstream)
        }
        ResolveTransportLeafError::MissingLeaf => tcp_failure(
            stage,
            prefixed_expected_outbound_leaf_error(invalid_leaf_stage, expected_leaf),
            None,
        ),
    })
}

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
    resolve_transport_leaf(source_dir, leaf, build).map_err(|error| match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            invalid_input_error(invalid_config, error)
        }
        ResolveTransportLeafError::MissingLeaf => {
            prefixed_expected_outbound_leaf_error(invalid_leaf_stage, expected_leaf)
        }
    })
}

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
    let upstream = proxy_upstream_endpoint(leaf);
    resolve_transport_leaf(source_dir, leaf, build).map_err(|error| match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            udp_flow_failure(stage, invalid_input_error(invalid_config, error), upstream)
        }
        ResolveTransportLeafError::MissingLeaf => {
            udp_flow_failure(stage, expected_outbound_leaf_error(expected_leaf), None)
        }
    })
}

fn last_udp_transport_leaf<'a, 'chain, T, E, F>(
    chain: &'chain [ResolvedLeafOutbound<'a>],
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
    resolve_last_transport_leaf(chain, source_dir, build).map_err(|error| match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            let upstream = chain.last().and_then(ResolvedLeafOutbound::proxy_endpoint);
            udp_flow_failure(stage, invalid_input_error(invalid_config, error), upstream)
        }
        ResolveTransportLeafError::MissingLeaf => {
            udp_flow_failure(stage, expected_outbound_leaf_error(expected_leaf), None)
        }
    })
}

pub(crate) fn managed_stream_udp_handler_for_bridge<TBridge>() -> Box<dyn ManagedStreamFlowHandler>
where
    TBridge: ProtocolManagedStreamUdpBridgeHandlerMetadata,
    TBridge::Resume: ManagedStreamFlowConnector,
{
    managed_stream_handler_box::<TBridge::Resume>(TBridge::managed_stream_flow_stages())
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
    let proxy = ctx.proxy();
    let transport_leaf = tcp_transport_leaf(
        leaf,
        proxy.config.source_dir(),
        A::Bridge::TCP_CONNECT_STAGE,
        A::Bridge::TCP_INVALID_CONNECT_CONFIG,
        A::Bridge::TCP_INVALID_CONNECT_LEAF_STAGE,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
    )?;
    let endpoint = transport_leaf_endpoint(&transport_leaf);
    let opened = open_tcp_transport_bridge_stream(adapter.bridge(), session, &transport_leaf, {
        move |server, port| proxy.connect_upstream_host_owned(server.to_owned(), port)
    })
    .await
    .map_err(|error| {
        tcp_failure(
            A::Bridge::TCP_CONNECT_STAGE,
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
    let transport_leaf = tcp_relay_transport_leaf(
        leaf,
        ctx.proxy().config.source_dir(),
        A::Bridge::TCP_INVALID_RELAY_CONFIG,
        A::Bridge::TCP_INVALID_RELAY_LEAF_STAGE,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
    )?;
    open_tcp_transport_bridge_relay_hop(adapter.bridge(), stream, session, &transport_leaf).await
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
    let proxy = ctx.proxy();
    let transport_leaf = udp_transport_leaf(
        leaf,
        proxy.config.source_dir(),
        A::Bridge::UDP_DIRECT_STAGE,
        A::Bridge::UDP_INVALID_CONFIG,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
    )?;
    let endpoint = transport_leaf_endpoint(&transport_leaf);
    start_direct_managed_stream_packet(
        dispatch,
        proxy,
        endpoint.tag,
        session,
        endpoint.server,
        endpoint.port,
        adapter.bridge().direct_udp_resume_for_leaf(&transport_leaf),
        payload,
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
    let proxy = ctx.proxy();
    let transport_leaf = udp_transport_leaf(
        leaf,
        proxy.config.source_dir(),
        A::Bridge::UDP_RELAY_FINAL_STAGE,
        A::Bridge::UDP_INVALID_CONFIG,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
    )?;
    let endpoint = transport_leaf_endpoint(&transport_leaf);
    transport_leaf
        .validate_udp_relay_final_hop()
        .map_err(|error| {
            udp_flow_failure(
                "udp_relay_final_transport",
                error,
                Some((endpoint.server, endpoint.port)),
            )
        })?;
    start_relay_managed_stream_packet(
        dispatch,
        Some(proxy),
        endpoint.tag,
        session,
        carrier,
        None,
        endpoint.server,
        endpoint.port,
        adapter
            .bridge()
            .relay_final_hop_udp_resume_for_leaf(&transport_leaf),
        payload,
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
    udp_transport_leaf(
        leaf,
        None,
        A::Bridge::UDP_RELAY_CAPABILITY_STAGE,
        A::Bridge::UDP_INVALID_CONFIG,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
    )
    .is_ok_and(|transport_leaf| {
        adapter
            .bridge()
            .udp_relay_needs_two_streams_for_leaf(&transport_leaf)
    })
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
    let proxy = ctx.proxy();
    let transport_leaf = last_udp_transport_leaf(
        chain,
        proxy.config.source_dir(),
        A::Bridge::UDP_RELAY_CAPABILITY_STAGE,
        A::Bridge::UDP_INVALID_CONFIG,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
    )?;
    let endpoint = transport_leaf_endpoint(&transport_leaf);
    let resume = adapter
        .bridge()
        .relay_two_stream_udp_resume_for_leaf(&transport_leaf);
    start_relay_two_stream_managed_flow(
        dispatch,
        proxy,
        session,
        chain,
        endpoint.tag,
        endpoint.server,
        endpoint.port,
        A::Bridge::UDP_RELAY_CHAIN_STAGE,
        |post_stream, get_stream| {
            open_relay_two_stream_udp_transport(&transport_leaf, post_stream, get_stream)
        },
        resume,
        payload,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn start_relay_two_stream_managed_flow<T, FBuild, FBuildFut>(
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    chain: &[ResolvedLeafOutbound<'_>],
    tag: &str,
    server: &str,
    port: u16,
    paired_stage: &'static str,
    build_transport: FBuild,
    resume: T,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    T: std::any::Any + Send + Sync + std::fmt::Debug,
    FBuild: FnOnce(TcpRelayStream, TcpRelayStream) -> FBuildFut,
    FBuildFut: Future<Output = Result<TcpRelayStream, EngineError>>,
{
    let chain_post = chain.to_vec();
    let chain_get = chain.to_vec();
    let (post_carrier, _) = proxy
        .dispatch_tcp_relay_prefix(chain_post)
        .await
        .map_err(relay_chain_flow_failure)?;
    let (get_carrier, _) = proxy
        .dispatch_tcp_relay_prefix(chain_get)
        .await
        .map_err(relay_chain_flow_failure)?;
    let paired_stream = build_transport(post_carrier.stream, get_carrier.stream)
        .await
        .map_err(|error| FlowFailure {
            stage: paired_stage,
            error,
            upstream: None,
        })?;

    start_relay_managed_stream_packet(
        dispatch,
        Some(proxy),
        tag,
        session,
        RelayCarrier {
            stream: paired_stream,
            server: server.to_string(),
            port,
        },
        None,
        server,
        port,
        resume,
        payload,
    )
    .await
}
