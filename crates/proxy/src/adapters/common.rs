use std::any::Any;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
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

use crate::protocol_registry::{OutboundAdapterContext, OutboundLeafRuntime, UdpAdapterContext};
use crate::runtime::orchestration::{OutboundEndpoint, TcpPathCategory};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::{
    managed_stream_handler_box, start_direct_managed_stream_packet,
    start_relay_managed_stream_packet, ManagedStreamFlowConnector, ManagedStreamFlowHandler,
};
use crate::runtime::Proxy;
use crate::transport::RelayCarrier;
use crate::transport::{EstablishedTcpOutbound, StreamTraffic, TcpOutboundFailure, TcpRelayStream};

fn owned_upstream_endpoint((server, port): (&str, u16)) -> (String, u16) {
    (server.to_string(), port)
}

pub(crate) trait NamedProtocolAdapter {
    const PROTOCOL_NAME: &'static str;
    const FEATURE_NAME: &'static str;
    const HAS_INBOUND: bool = true;
    const HAS_OUTBOUND: bool = true;
    const CLAIMS_RUNTIME_LEAF: bool = true;
}

pub(crate) trait ProtocolTransportBridgeAdapter: NamedProtocolAdapter {
    type Bridge;

    const TCP_PATH: TcpPathCategory;

    fn bridge(&self) -> &Self::Bridge;
}

pub(super) fn invalid_input_error(
    stage: &'static str,
    error: impl std::fmt::Display,
) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
    ))
}

pub(super) fn expected_outbound_leaf_error(message: &'static str) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message,
    ))
}

pub(super) fn prefixed_expected_outbound_leaf_error(
    stage: &'static str,
    message: &'static str,
) -> EngineError {
    invalid_input_error(stage, message)
}

pub(super) fn tcp_failure(
    stage: &'static str,
    error: EngineError,
    upstream: Option<(&str, u16)>,
) -> TcpOutboundFailure {
    TcpOutboundFailure {
        stage,
        error,
        upstream_endpoint: upstream.map(owned_upstream_endpoint),
    }
}

pub(super) fn udp_flow_failure(
    stage: &'static str,
    error: EngineError,
    upstream: Option<(&str, u16)>,
) -> FlowFailure {
    FlowFailure {
        stage,
        error,
        upstream: upstream.map(owned_upstream_endpoint),
    }
}

pub(super) fn relay_chain_flow_failure(failure: TcpOutboundFailure) -> FlowFailure {
    FlowFailure {
        stage: failure.stage,
        error: failure.error,
        upstream: failure.upstream_endpoint,
    }
}

pub(super) fn protocol_leaf_runtime<'a>(
    tag: &'a str,
    server: &'a str,
    port: u16,
    tcp_path: TcpPathCategory,
) -> OutboundLeafRuntime<'a> {
    OutboundLeafRuntime {
        tcp_path,
        health_tag: Some(tag),
        endpoint: Some(OutboundEndpoint { server, port }),
        kernel_tag: None,
        udp_policy_tag: Some(tag),
    }
}

pub(crate) fn named_protocol_claims_runtime_leaf<A>(leaf: &ResolvedLeafOutbound<'_>) -> bool
where
    A: NamedProtocolAdapter,
{
    A::CLAIMS_RUNTIME_LEAF && leaf.protocol_name() == A::PROTOCOL_NAME
}

pub(crate) fn named_protocol_supports_inbound<A>(config: &InboundProtocolConfig) -> bool
where
    A: NamedProtocolAdapter,
{
    A::HAS_INBOUND && config.protocol_name() == A::PROTOCOL_NAME
}

pub(crate) fn named_protocol_supports_outbound<A>(config: &OutboundProtocolConfig) -> bool
where
    A: NamedProtocolAdapter,
{
    A::HAS_OUTBOUND && config.protocol_name() == A::PROTOCOL_NAME
}

pub(crate) fn transport_bridge_adapter_claims_runtime_leaf<A>(
    leaf: &ResolvedLeafOutbound<'_>,
) -> bool
where
    A: ProtocolTransportBridgeAdapter,
{
    named_protocol_claims_runtime_leaf::<A>(leaf)
}

pub(crate) fn transport_bridge_adapter_leaf_runtime<'a, A>(
    leaf: &ResolvedLeafOutbound<'a>,
) -> Option<OutboundLeafRuntime<'a>>
where
    A: ProtocolTransportBridgeAdapter,
{
    proxy_leaf_runtime(leaf, A::TCP_PATH)
}

fn proxy_upstream_endpoint<'a>(leaf: &'a ResolvedLeafOutbound<'a>) -> Option<(&'a str, u16)> {
    leaf.proxy_endpoint()
}

pub(crate) fn managed_stream_udp_handler_for_bridge<TBridge>() -> Box<dyn ManagedStreamFlowHandler>
where
    TBridge: ProtocolManagedStreamUdpBridgeHandlerMetadata,
    TBridge::Resume: ManagedStreamFlowConnector,
{
    managed_stream_handler_box::<TBridge::Resume>(TBridge::managed_stream_flow_stages())
}

pub(crate) fn transport_bridge_adapter_managed_stream_udp_handler<A>(
) -> Box<dyn ManagedStreamFlowHandler>
where
    A: ProtocolTransportBridgeAdapter,
    A::Bridge: ProtocolManagedStreamUdpBridgeHandlerMetadata,
    <A::Bridge as ProtocolManagedStreamUdpBridgeHandlerMetadata>::Resume:
        ManagedStreamFlowConnector,
{
    managed_stream_udp_handler_for_bridge::<A::Bridge>()
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
    A::Bridge: Clone
        + Send
        + Sync
        + 'static
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<
            <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: Sync,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
    <A::Bridge as ProtocolTcpTransportBridgeOps<
        <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
    >>::Opened: ProtocolTcpTransportOpenResult,
    FInspect: FnOnce(&StreamTraffic),
{
    let proxy = ctx.proxy();
    connect_proxied_tcp_transport_bridge(
        proxy,
        session,
        leaf,
        proxy.config.source_dir(),
        A::Bridge::TCP_CONNECT_STAGE,
        A::Bridge::TCP_INVALID_CONNECT_CONFIG,
        A::Bridge::TCP_INVALID_CONNECT_LEAF_STAGE,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
        adapter.bridge(),
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
    A::Bridge: Clone
        + Send
        + Sync
        + 'static
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<
            <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
        >,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: Sync,
    <A::Bridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    apply_tcp_transport_bridge_relay_hop(
        leaf,
        ctx.proxy().config.source_dir(),
        A::Bridge::TCP_INVALID_RELAY_CONFIG,
        A::Bridge::TCP_INVALID_RELAY_LEAF_STAGE,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
        adapter.bridge(),
        stream,
        session,
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
    let proxy = ctx.proxy();
    start_direct_managed_udp_transport_bridge_flow(
        dispatch,
        proxy,
        session,
        leaf,
        payload,
        proxy.config.source_dir(),
        A::Bridge::UDP_DIRECT_STAGE,
        A::Bridge::UDP_INVALID_CONFIG,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
        adapter.bridge(),
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
    start_proxy_relay_managed_udp_transport_bridge_flow(
        dispatch,
        proxy,
        session,
        carrier,
        leaf,
        payload,
        proxy.config.source_dir(),
        A::Bridge::UDP_RELAY_FINAL_STAGE,
        A::Bridge::UDP_INVALID_CONFIG,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
        adapter.bridge(),
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
    udp_transport_bridge_relay_needs_two_streams(
        leaf,
        None,
        A::Bridge::UDP_RELAY_CAPABILITY_STAGE,
        A::Bridge::UDP_INVALID_CONFIG,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
        adapter.bridge(),
    )
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
    start_relay_two_stream_managed_udp_transport_bridge_flow(
        dispatch,
        proxy,
        session,
        chain,
        payload,
        proxy.config.source_dir(),
        A::Bridge::UDP_RELAY_CAPABILITY_STAGE,
        A::Bridge::UDP_INVALID_CONFIG,
        A::Bridge::EXPECTED_OUTBOUND_LEAF,
        A::Bridge::UDP_RELAY_CHAIN_STAGE,
        |source_dir, leaf| adapter.bridge().resolve_transport_leaf(source_dir, leaf),
        adapter.bridge(),
    )
    .await
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
    build(source_dir, leaf)
        .map_err(|error| tcp_failure(stage, invalid_input_error(invalid_config, error), upstream))?
        .ok_or_else(|| {
            tcp_failure(
                stage,
                prefixed_expected_outbound_leaf_error(invalid_leaf_stage, expected_leaf),
                None,
            )
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
    build(source_dir, leaf)
        .map_err(|error| invalid_input_error(invalid_config, error))?
        .ok_or_else(|| prefixed_expected_outbound_leaf_error(invalid_leaf_stage, expected_leaf))
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
    build(source_dir, leaf)
        .map_err(|error| {
            udp_flow_failure(stage, invalid_input_error(invalid_config, error), upstream)
        })?
        .ok_or_else(|| udp_flow_failure(stage, expected_outbound_leaf_error(expected_leaf), None))
}

pub(super) async fn connect_proxied_tcp_transport_leaf<
    'a,
    TLeaf,
    TOpened,
    E,
    FBuild,
    FOpen,
    FInspect,
>(
    proxy: &Proxy,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'a>,
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    invalid_leaf_stage: &'static str,
    expected_leaf: &'static str,
    build: FBuild,
    open: FOpen,
    inspect_traffic: FInspect,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>
where
    TLeaf: ProtocolTransportLeaf,
    TOpened: ProtocolTcpTransportOpenResult,
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
    FOpen: for<'b> FnOnce(
        &'b TLeaf,
    ) -> Pin<
        Box<dyn Future<Output = Result<TOpened, EngineError>> + Send + 'b>,
    >,
    FInspect: FnOnce(&StreamTraffic),
{
    let transport_leaf = tcp_transport_leaf(
        leaf,
        source_dir,
        stage,
        invalid_config,
        invalid_leaf_stage,
        expected_leaf,
        build,
    )?;
    open_proxied_tcp_stream(
        proxy,
        session,
        transport_leaf.tag(),
        transport_leaf.server(),
        transport_leaf.port(),
        stage,
        open(&transport_leaf),
        TOpened::into_proxied_stream_parts,
        inspect_traffic,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn connect_proxied_tcp_transport_bridge<'a, TLeaf, TBridge, E, FBuild, FInspect>(
    proxy: &Proxy,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'a>,
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    invalid_leaf_stage: &'static str,
    expected_leaf: &'static str,
    build: FBuild,
    bridge: &TBridge,
    inspect_traffic: FInspect,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>
where
    TLeaf: ProtocolTransportLeaf + Sync,
    TBridge: ProtocolTcpTransportBridgeOps<TLeaf> + Clone + Send + 'static,
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
    FInspect: FnOnce(&StreamTraffic),
{
    let proxy = proxy.clone();
    let session = session.clone();
    let bridge = bridge.clone();
    connect_proxied_tcp_transport_leaf(
        &proxy,
        &session,
        leaf,
        source_dir,
        stage,
        invalid_config,
        invalid_leaf_stage,
        expected_leaf,
        build,
        |transport_leaf| {
            let proxy = proxy.clone();
            let session = session.clone();
            let bridge = bridge.clone();
            Box::pin(async move {
                bridge
                    .open_tcp_stream_for_leaf(&session, transport_leaf, {
                        let proxy = proxy.clone();
                        move |server, port| {
                            let proxy = proxy.clone();
                            let server = server.to_owned();
                            async move { proxy.connect_upstream_host_owned(server, port).await }
                        }
                    })
                    .await
            })
        },
        inspect_traffic,
    )
    .await
}

pub(super) async fn open_proxied_tcp_stream<TOpen, TOpened, FMap, FInspect>(
    proxy: &Proxy,
    session: &Session,
    tag: &str,
    server: &str,
    port: u16,
    stage: &'static str,
    open: TOpen,
    map_opened: FMap,
    inspect_traffic: FInspect,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>
where
    TOpen: Future<Output = Result<TOpened, EngineError>>,
    FMap: FnOnce(TOpened) -> (TcpRelayStream, StreamTraffic),
    FInspect: FnOnce(&StreamTraffic),
{
    let opened = open
        .await
        .map_err(|error| tcp_failure(stage, error, Some((server, port))))?;
    let (stream, traffic) = map_opened(opened);
    inspect_traffic(&traffic);
    if !traffic.is_empty() {
        proxy.record_session_outbound_traffic(session.id, traffic);
    }
    Ok(EstablishedTcpOutbound::proxied(tag, server, port, stream))
}

pub(super) async fn apply_tcp_transport_relay_hop<'a, TLeaf, E, FBuild, FOpen>(
    leaf: &ResolvedLeafOutbound<'a>,
    source_dir: Option<&Path>,
    invalid_config: &'static str,
    invalid_leaf_stage: &'static str,
    expected_leaf: &'static str,
    build: FBuild,
    open: FOpen,
) -> Result<TcpRelayStream, EngineError>
where
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
    FOpen: for<'b> FnOnce(
        &'b TLeaf,
    ) -> Pin<
        Box<dyn Future<Output = Result<TcpRelayStream, EngineError>> + Send + 'b>,
    >,
{
    let transport_leaf = tcp_relay_transport_leaf(
        leaf,
        source_dir,
        invalid_config,
        invalid_leaf_stage,
        expected_leaf,
        build,
    )?;
    open(&transport_leaf).await
}

pub(super) async fn apply_tcp_transport_bridge_relay_hop<'a, TLeaf, TBridge, E, FBuild>(
    leaf: &ResolvedLeafOutbound<'a>,
    source_dir: Option<&Path>,
    invalid_config: &'static str,
    invalid_leaf_stage: &'static str,
    expected_leaf: &'static str,
    build: FBuild,
    bridge: &TBridge,
    stream: TcpRelayStream,
    session: &Session,
) -> Result<TcpRelayStream, EngineError>
where
    TLeaf: Sync,
    TBridge: ProtocolTcpTransportBridgeOps<TLeaf> + Clone + Send + 'static,
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
{
    let session = session.clone();
    let bridge = bridge.clone();
    apply_tcp_transport_relay_hop(
        leaf,
        source_dir,
        invalid_config,
        invalid_leaf_stage,
        expected_leaf,
        build,
        |transport_leaf| {
            let session = session.clone();
            let bridge = bridge.clone();
            Box::pin(async move {
                bridge
                    .open_tcp_relay_hop_for_leaf(stream, &session, transport_leaf)
                    .await
            })
        },
    )
    .await
}

pub(super) async fn start_direct_managed_udp_transport_flow<
    'a,
    TLeaf,
    TResume,
    E,
    FBuild,
    FResume,
>(
    dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'a>,
    payload: &[u8],
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
    build_leaf: FBuild,
    build_resume: FResume,
) -> Result<FlowStartResult, FlowFailure>
where
    TLeaf: ProtocolTransportLeaf,
    TResume: Any + Send + Sync + std::fmt::Debug,
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
    FResume: Fn(&TLeaf) -> TResume,
{
    let transport_leaf = udp_transport_leaf(
        leaf,
        source_dir,
        stage,
        invalid_config,
        expected_leaf,
        build_leaf,
    )?;
    start_direct_managed_stream_flow(
        dispatch,
        proxy,
        session,
        transport_leaf.tag(),
        transport_leaf.server(),
        transport_leaf.port(),
        build_resume(&transport_leaf),
        payload,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn start_direct_managed_udp_transport_bridge_flow<'a, TLeaf, TBridge, E, FBuild>(
    dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'a>,
    payload: &[u8],
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
    build_leaf: FBuild,
    bridge: &TBridge,
) -> Result<FlowStartResult, FlowFailure>
where
    TLeaf: ProtocolTransportLeaf,
    TBridge: ProtocolManagedStreamUdpBridgeOps<TLeaf>,
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
{
    start_direct_managed_udp_transport_flow(
        dispatch,
        proxy,
        session,
        leaf,
        payload,
        source_dir,
        stage,
        invalid_config,
        expected_leaf,
        build_leaf,
        |transport_leaf| bridge.direct_udp_resume_for_leaf(transport_leaf),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn start_proxy_relay_managed_udp_transport_flow<
    'a,
    TLeaf,
    TResume,
    E,
    FBuild,
    FResume,
>(
    dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    carrier: RelayCarrier,
    leaf: &ResolvedLeafOutbound<'a>,
    payload: &[u8],
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
    build_leaf: FBuild,
    build_resume: FResume,
) -> Result<FlowStartResult, FlowFailure>
where
    TLeaf: ProtocolTransportLeaf,
    TResume: Any + Send + Sync + std::fmt::Debug,
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
    FResume: Fn(&TLeaf) -> TResume,
{
    let transport_leaf = udp_transport_leaf(
        leaf,
        source_dir,
        stage,
        invalid_config,
        expected_leaf,
        build_leaf,
    )?;
    transport_leaf
        .validate_udp_relay_final_hop()
        .map_err(|error| {
            udp_flow_failure(
                "udp_relay_final_transport",
                error,
                Some((transport_leaf.server(), transport_leaf.port())),
            )
        })?;
    start_relay_managed_stream_flow(
        dispatch,
        Some(proxy),
        session,
        carrier,
        None,
        transport_leaf.tag(),
        transport_leaf.server(),
        transport_leaf.port(),
        build_resume(&transport_leaf),
        payload,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn start_proxy_relay_managed_udp_transport_bridge_flow<
    'a,
    TLeaf,
    TBridge,
    E,
    FBuild,
>(
    dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    carrier: RelayCarrier,
    leaf: &ResolvedLeafOutbound<'a>,
    payload: &[u8],
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
    build_leaf: FBuild,
    bridge: &TBridge,
) -> Result<FlowStartResult, FlowFailure>
where
    TLeaf: ProtocolTransportLeaf,
    TBridge: ProtocolManagedStreamUdpBridgeOps<TLeaf>,
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
{
    start_proxy_relay_managed_udp_transport_flow(
        dispatch,
        proxy,
        session,
        carrier,
        leaf,
        payload,
        source_dir,
        stage,
        invalid_config,
        expected_leaf,
        build_leaf,
        |transport_leaf| bridge.relay_final_hop_udp_resume_for_leaf(transport_leaf),
    )
    .await
}

pub(super) async fn start_direct_managed_stream_flow<T>(
    dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    tag: &str,
    server: &str,
    port: u16,
    resume: T,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    start_direct_managed_stream_packet(dispatch, proxy, tag, session, server, port, resume, payload)
        .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn start_relay_managed_stream_flow<T>(
    dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
    proxy: Option<&Proxy>,
    session: &Session,
    carrier: RelayCarrier,
    tls_server_name: Option<&str>,
    tag: &str,
    server: &str,
    port: u16,
    resume: T,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    start_relay_managed_stream_packet(
        dispatch,
        proxy,
        tag,
        session,
        carrier,
        tls_server_name,
        server,
        port,
        resume,
        payload,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn start_relay_two_stream_managed_flow<T, FBuild, FBuildFut>(
    dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
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
    T: Any + Send + Sync + std::fmt::Debug,
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

    start_relay_managed_stream_flow(
        dispatch,
        Some(proxy),
        session,
        RelayCarrier {
            stream: paired_stream,
            server: server.to_string(),
            port,
        },
        None,
        tag,
        server,
        port,
        resume,
        payload,
    )
    .await
}

pub(super) fn udp_transport_bridge_relay_needs_two_streams<'a, TLeaf, TBridge, E, FBuild>(
    leaf: &ResolvedLeafOutbound<'a>,
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
    build: FBuild,
    bridge: &TBridge,
) -> bool
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf,
    TBridge: ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
{
    udp_transport_leaf(
        leaf,
        source_dir,
        stage,
        invalid_config,
        expected_leaf,
        build,
    )
    .is_ok_and(|transport_leaf| bridge.udp_relay_needs_two_streams_for_leaf(&transport_leaf))
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn start_relay_two_stream_managed_udp_transport_bridge_flow<
    'a,
    'chain,
    TLeaf,
    TBridge,
    E,
    FBuild,
>(
    dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    chain: &'chain [ResolvedLeafOutbound<'a>],
    payload: &[u8],
    source_dir: Option<&Path>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
    paired_stage: &'static str,
    build: FBuild,
    bridge: &TBridge,
) -> Result<FlowStartResult, FlowFailure>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf,
    TBridge: ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
    E: std::fmt::Display,
    FBuild: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<TLeaf>, E>,
{
    let transport_leaf = last_udp_transport_leaf(
        chain,
        source_dir,
        stage,
        invalid_config,
        expected_leaf,
        build,
    )?;
    start_relay_two_stream_managed_flow(
        dispatch,
        proxy,
        session,
        chain,
        transport_leaf.tag(),
        transport_leaf.server(),
        transport_leaf.port(),
        paired_stage,
        |post_stream, get_stream| {
            transport_leaf.open_relay_two_stream_udp_transport(post_stream, get_stream)
        },
        bridge.relay_two_stream_udp_resume_for_leaf(&transport_leaf),
        payload,
    )
    .await
}

pub(super) fn last_udp_transport_leaf<'a, 'chain, T, E, F>(
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
    let relay_leaf = chain.last().ok_or_else(|| {
        udp_flow_failure(stage, expected_outbound_leaf_error(expected_leaf), None)
    })?;
    udp_transport_leaf(
        relay_leaf,
        source_dir,
        stage,
        invalid_config,
        expected_leaf,
        build,
    )
}

/// Build a `TcpOutboundFailure` for the impossible case where an adapter's
/// `connect_tcp` receives a leaf variant it did not claim.
///
/// `claims_outbound_leaf` guarantees the variant matches before the runtime
/// dispatches `connect_tcp`, so this only fires on a programming error.
pub(super) fn unreachable_leaf(
    adapter: &'static str,
    _leaf: &ResolvedLeafOutbound<'_>,
) -> TcpOutboundFailure {
    tcp_failure(
        "outbound_leaf_mismatch",
        EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching outbound leaf"
        ))),
        None,
    )
}

/// Same as [`unreachable_leaf`] but for the UDP `start_udp_flow` path.
pub(super) fn unreachable_udp_leaf(
    adapter: &'static str,
    _leaf: &ResolvedLeafOutbound<'_>,
) -> FlowFailure {
    udp_flow_failure(
        "udp_leaf_mismatch",
        EngineError::Io(std::io::Error::other(format!(
            "{adapter} adapter received a non-matching UDP leaf"
        ))),
        None,
    )
}

pub(super) fn direct_leaf_runtime<'a>(
    leaf: &ResolvedLeafOutbound<'a>,
) -> Option<OutboundLeafRuntime<'a>> {
    match leaf {
        ResolvedLeafOutbound::Direct { tag } => Some(OutboundLeafRuntime {
            tcp_path: TcpPathCategory::Direct,
            health_tag: None,
            endpoint: None,
            kernel_tag: *tag,
            udp_policy_tag: *tag,
        }),
        _ => None,
    }
}

pub(super) fn proxy_leaf_runtime<'a>(
    leaf: &ResolvedLeafOutbound<'a>,
    tcp_path: TcpPathCategory,
) -> Option<OutboundLeafRuntime<'a>> {
    let tag = leaf.tag()?;
    let (server, port) = leaf.proxy_endpoint()?;

    Some(protocol_leaf_runtime(tag, server, port, tcp_path))
}
