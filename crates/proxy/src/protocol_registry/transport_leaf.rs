use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use zero_core::Session;
use zero_engine::EngineError;
use zero_engine::ResolvedLeafOutbound;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use zero_transport::managed_udp::ProtocolManagedStreamUdpBridgeOps;
#[cfg(feature = "vless")]
use zero_transport::managed_udp::ProtocolRelayTwoStreamManagedUdpBridgeOps;
#[cfg(feature = "vless")]
use zero_transport::outbound_leaf::{
    open_prepared_relay_two_stream_udp_transport, prepared_relay_two_stream_udp_resume,
    prepared_udp_relay_needs_two_streams, ProtocolRelayTwoStreamTransportLeaf,
    ProtocolRelayTwoStreamUdpTransportBridgeMetadata,
};
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use zero_transport::outbound_leaf::{
    prepared_direct_udp_resume, prepared_relay_final_hop_udp_resume,
    ProtocolUdpTransportBridgeMetadata,
};
use zero_transport::outbound_leaf::{
    PreparedTransportBridgeLeaf, ProtocolTcpTransportBridgeMetadata, ProtocolTcpTransportBridgeOps,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
};

use super::UdpAdapterContext;
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use crate::runtime::udp_flow::managed::bridge::{
    start_direct_managed_stream_packet, start_relay_managed_stream_packet,
    ManagedStreamPacketRelay, ManagedStreamPacketStartBridge,
};
use crate::runtime::Proxy;
use crate::transport::RelayCarrier;
#[cfg(feature = "vless")]
use crate::transport::TcpOutboundFailure;

pub(crate) enum ResolveTransportLeafError<E> {
    InvalidConfig(E),
    MissingLeaf,
}

pub(crate) trait ProtocolTransportLeafResolver<'a> {
    type TransportLeaf: ProtocolTransportLeaf + 'a;
    type ResolveError: std::fmt::Display;

    fn resolve_transport_leaf(
        &self,
        source_dir: Option<&Path>,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<Option<Self::TransportLeaf>, Self::ResolveError>;
}

pub(crate) fn prepare_transport_bridge_leaf<'a, TBridge>(
    bridge: &TBridge,
    source_dir: Option<&Path>,
    leaf: &ResolvedLeafOutbound<'a>,
) -> Result<
    PreparedTransportBridgeLeaf<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    ResolveTransportLeafError<<TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError>,
>
where
    TBridge: ProtocolTransportLeafResolver<'a>,
{
    bridge
        .resolve_transport_leaf(source_dir, leaf)
        .map_err(ResolveTransportLeafError::InvalidConfig)?
        .map(PreparedTransportBridgeLeaf::new)
        .ok_or(ResolveTransportLeafError::MissingLeaf)
}

pub(crate) fn prepare_last_transport_bridge_leaf<'a, TBridge>(
    bridge: &TBridge,
    chain: &[ResolvedLeafOutbound<'a>],
    source_dir: Option<&Path>,
) -> Result<
    PreparedTransportBridgeLeaf<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    ResolveTransportLeafError<<TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError>,
>
where
    TBridge: ProtocolTransportLeafResolver<'a>,
{
    let leaf = chain.last().ok_or(ResolveTransportLeafError::MissingLeaf)?;
    prepare_transport_bridge_leaf(bridge, source_dir, leaf)
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) fn prepare_transport_bridge_tcp_connect<'a, TBridge>(
    bridge: &'a TBridge,
    source_dir: Option<&Path>,
    leaf: &'a ResolvedLeafOutbound<'a>,
) -> Result<
    Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpConnectOperation + 'a>,
    crate::transport::TcpOutboundFailure,
>
where
    TBridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf:
        ProtocolTransportLeaf + Send + Sync,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
    TBridge::Opened: ProtocolTcpTransportOpenResult,
{
    let prepared = prepare_transport_bridge_leaf(bridge, source_dir, leaf)
        .map_err(|error| connect_prepare_failure::<TBridge>(leaf, error))?;
    Ok(Box::new(
        crate::runtime::tcp_dispatch::operation::TransportBridgeTcpConnectOperation {
            bridge,
            prepared,
        },
    ))
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) fn prepare_transport_bridge_tcp_relay<'a, TBridge>(
    bridge: &'a TBridge,
    source_dir: Option<&Path>,
    leaf: &'a ResolvedLeafOutbound<'a>,
) -> Result<
    Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpRelayOperation + 'a>,
    EngineError,
>
where
    TBridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: Send + Sync,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    let prepared = prepare_transport_bridge_leaf(bridge, source_dir, leaf)
        .map_err(|error| relay_prepare_error::<TBridge, _>(error))?;
    Ok(Box::new(
        crate::runtime::tcp_dispatch::operation::TransportBridgeTcpRelayOperation {
            bridge,
            prepared,
        },
    ))
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
fn connect_prepare_failure<TBridge>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<impl std::fmt::Display>,
) -> crate::transport::TcpOutboundFailure
where
    TBridge: ProtocolTcpTransportBridgeMetadata,
{
    let (stage, error, upstream_endpoint) = match error {
        ResolveTransportLeafError::InvalidConfig(error) => (
            TBridge::TCP_CONNECT_STAGE,
            invalid_input(TBridge::TCP_INVALID_CONNECT_CONFIG, error),
            leaf.proxy_endpoint()
                .map(|(server, port)| (server.to_owned(), port)),
        ),
        ResolveTransportLeafError::MissingLeaf => (
            TBridge::TCP_CONNECT_STAGE,
            invalid_input(
                TBridge::TCP_INVALID_CONNECT_LEAF_STAGE,
                TBridge::EXPECTED_OUTBOUND_LEAF,
            ),
            None,
        ),
    };
    crate::transport::TcpOutboundFailure {
        stage,
        error,
        upstream_endpoint,
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
fn relay_prepare_error<TBridge, E>(error: ResolveTransportLeafError<E>) -> EngineError
where
    TBridge: ProtocolTcpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            invalid_input(TBridge::TCP_INVALID_RELAY_CONFIG, error)
        }
        ResolveTransportLeafError::MissingLeaf => invalid_input(
            TBridge::TCP_INVALID_RELAY_LEAF_STAGE,
            TBridge::EXPECTED_OUTBOUND_LEAF,
        ),
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
fn invalid_input(stage: &'static str, error: impl std::fmt::Display) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
    ))
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
fn expected_outbound_leaf_error(message: &'static str) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        message,
    ))
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
fn udp_flow_failure(
    stage: &'static str,
    error: EngineError,
    upstream: Option<(&str, u16)>,
) -> FlowFailure {
    FlowFailure {
        stage,
        error,
        upstream: upstream.map(|(server, port)| (server.to_string(), port)),
    }
}

#[cfg(feature = "vless")]
fn relay_chain_flow_failure(failure: TcpOutboundFailure) -> FlowFailure {
    FlowFailure {
        stage: failure.stage,
        error: failure.error,
        upstream: failure.upstream_endpoint,
    }
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
fn udp_prepare_failure<E>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<E>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
) -> FlowFailure
where
    E: std::fmt::Display,
{
    let upstream = leaf.proxy_endpoint();
    match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            udp_flow_failure(stage, invalid_input(invalid_config, error), upstream)
        }
        ResolveTransportLeafError::MissingLeaf => {
            udp_flow_failure(stage, expected_outbound_leaf_error(expected_leaf), None)
        }
    }
}

#[cfg(feature = "vless")]
fn last_udp_prepare_failure<E>(
    chain: &[ResolvedLeafOutbound<'_>],
    error: ResolveTransportLeafError<E>,
    stage: &'static str,
    invalid_config: &'static str,
    expected_leaf: &'static str,
) -> FlowFailure
where
    E: std::fmt::Display,
{
    match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            let upstream = chain.last().and_then(|leaf| leaf.proxy_endpoint());
            udp_flow_failure(stage, invalid_input(invalid_config, error), upstream)
        }
        ResolveTransportLeafError::MissingLeaf => {
            udp_flow_failure(stage, expected_outbound_leaf_error(expected_leaf), None)
        }
    }
}

#[cfg(feature = "vless")]
pub(crate) fn transport_bridge_udp_relay_needs_two_streams<'a, TBridge>(
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

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) enum PreparedTransportUdpOperation<'a, 'leaf> {
    Direct {
        leaf: &'leaf ResolvedLeafOutbound<'a>,
    },
    RelayFinalHop {
        carrier: RelayCarrier,
        leaf: &'leaf ResolvedLeafOutbound<'a>,
    },
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) struct TransportBridgeUdpOperation<'a, TBridge> {
    pub(crate) bridge: &'a TBridge,
    pub(crate) operation: PreparedTransportUdpOperation<'a, 'a>,
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
impl<'leaf, TBridge> PreparedUdpFlowOperation for TransportBridgeUdpOperation<'leaf, TBridge>
where
    TBridge: Send
        + Sync
        + ProtocolUdpTransportBridgeMetadata
        + for<'resolve> ProtocolTransportLeafResolver<'resolve>,
    for<'resolve> TBridge: ProtocolManagedStreamUdpBridgeOps<
        <TBridge as ProtocolTransportLeafResolver<'resolve>>::TransportLeaf,
    >,
    for<'resolve> <TBridge as ProtocolTransportLeafResolver<'resolve>>::TransportLeaf:
        ProtocolTransportLeaf + Send,
    for<'resolve> <TBridge as ProtocolTransportLeafResolver<'resolve>>::ResolveError:
        std::fmt::Display,
{
    fn execute<'a>(
        self: Box<Self>,
        dispatch: &'a mut UdpDispatch,
        ctx: UdpAdapterContext<'a>,
        session: &'a Session,
        payload: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<FlowStartResult, FlowFailure>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            execute_transport_udp_operation(
                self.bridge,
                dispatch,
                ctx.proxy(),
                session,
                payload,
                self.operation,
            )
            .await
        })
    }
}

#[cfg(feature = "vless")]
pub(crate) struct PreparedRelayTwoStreamUdpOperation<'a> {
    pub(crate) chain: Vec<ResolvedLeafOutbound<'a>>,
}

#[cfg(feature = "vless")]
pub(crate) struct RelayTwoStreamUdpOperation<'a, TBridge> {
    pub(crate) bridge: &'a TBridge,
    pub(crate) chain: Vec<ResolvedLeafOutbound<'a>>,
}

#[cfg(feature = "vless")]
impl<'leaf, TBridge> PreparedUdpFlowOperation for RelayTwoStreamUdpOperation<'leaf, TBridge>
where
    TBridge: Send
        + Sync
        + ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + for<'resolve> ProtocolTransportLeafResolver<'resolve>,
    for<'resolve> TBridge: ProtocolRelayTwoStreamManagedUdpBridgeOps<
        <TBridge as ProtocolTransportLeafResolver<'resolve>>::TransportLeaf,
    >,
    for<'resolve> <TBridge as ProtocolTransportLeafResolver<'resolve>>::TransportLeaf:
        ProtocolRelayTwoStreamTransportLeaf + Send + Sync,
    for<'resolve> <TBridge as ProtocolTransportLeafResolver<'resolve>>::ResolveError:
        std::fmt::Display,
{
    fn execute<'a>(
        self: Box<Self>,
        dispatch: &'a mut UdpDispatch,
        ctx: UdpAdapterContext<'a>,
        session: &'a Session,
        payload: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<FlowStartResult, FlowFailure>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            execute_relay_two_stream_udp_operation(
                self.bridge,
                dispatch,
                ctx.proxy(),
                session,
                payload,
                PreparedRelayTwoStreamUdpOperation { chain: self.chain },
            )
            .await
        })
    }
}

#[cfg(feature = "vless")]
pub(crate) async fn execute_relay_two_stream_udp_operation<'a, TBridge>(
    bridge: &TBridge,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    payload: &[u8],
    operation: PreparedRelayTwoStreamUdpOperation<'a>,
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
    let mut context = dispatch.flow_start_context();
    let prepared =
        prepare_last_transport_bridge_leaf(bridge, &operation.chain, proxy.config.source_dir())
            .map_err(|error| {
                last_udp_prepare_failure(
                    &operation.chain,
                    error,
                    TBridge::UDP_RELAY_CAPABILITY_STAGE,
                    TBridge::UDP_INVALID_CONFIG,
                    TBridge::EXPECTED_OUTBOUND_LEAF,
                )
            })?;
    let endpoint = prepared.endpoint();
    let resume = prepared_relay_two_stream_udp_resume(bridge, &prepared);
    let chain_post = operation.chain.clone();
    let chain_get = operation.chain;
    let (post_carrier, _) = proxy
        .dispatch_tcp_relay_prefix(chain_post)
        .await
        .map_err(relay_chain_flow_failure)?;
    let (get_carrier, _) = proxy
        .dispatch_tcp_relay_prefix(chain_get)
        .await
        .map_err(relay_chain_flow_failure)?;
    let paired_stream = open_prepared_relay_two_stream_udp_transport(
        &prepared,
        post_carrier.stream,
        get_carrier.stream,
    )
    .await
    .map_err(|error| FlowFailure {
        stage: TBridge::UDP_RELAY_CHAIN_STAGE,
        error: error.into(),
        upstream: None,
    })?;

    start_relay_managed_stream_packet(
        &mut context,
        ManagedStreamPacketStartBridge::relay(
            Some(proxy),
            endpoint.tag,
            session,
            ManagedStreamPacketRelay {
                carrier: RelayCarrier {
                    stream: paired_stream,
                    server: endpoint.server.to_string(),
                    port: endpoint.port,
                },
                tls_server_name: None,
            },
            (endpoint.server, endpoint.port),
            resume,
            payload,
        ),
    )
    .await
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) async fn execute_transport_udp_operation<'a, TBridge>(
    bridge: &TBridge,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    payload: &[u8],
    operation: PreparedTransportUdpOperation<'a, '_>,
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
    match operation {
        PreparedTransportUdpOperation::Direct { leaf } => {
            let mut context = dispatch.flow_start_context();
            let prepared = prepare_transport_bridge_leaf(bridge, proxy.config.source_dir(), leaf)
                .map_err(|error| {
                udp_prepare_failure(
                    leaf,
                    error,
                    TBridge::UDP_DIRECT_STAGE,
                    TBridge::UDP_INVALID_CONFIG,
                    TBridge::EXPECTED_OUTBOUND_LEAF,
                )
            })?;
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
        PreparedTransportUdpOperation::RelayFinalHop { carrier, leaf } => {
            let mut context = dispatch.flow_start_context();
            let prepared = prepare_transport_bridge_leaf(bridge, proxy.config.source_dir(), leaf)
                .map_err(|error| {
                udp_prepare_failure(
                    leaf,
                    error,
                    TBridge::UDP_RELAY_FINAL_STAGE,
                    TBridge::UDP_INVALID_CONFIG,
                    TBridge::EXPECTED_OUTBOUND_LEAF,
                )
            })?;
            let endpoint = prepared.endpoint();
            prepared.validate_udp_relay_final_hop().map_err(|error| {
                udp_flow_failure(
                    "udp_relay_final_transport",
                    error.into(),
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
    }
}
