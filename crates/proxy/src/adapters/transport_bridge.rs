use std::path::Path;

use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_transport::outbound_leaf::{
    PreparedTransportBridgeLeaf, ProtocolRelayTwoStreamUdpTransportBridgeMetadata,
    ProtocolTcpTransportBridgeMetadata, ProtocolTcpTransportBridgeOps,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf, ProtocolUdpTransportBridgeMetadata,
};

use crate::protocol_registry::{
    prepare_transport_bridge_tcp_connect, prepare_transport_bridge_tcp_relay,
    ClaimedTcpOutboundLeaf,
};
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
use crate::runtime::udp_dispatch::FlowFailure;
use crate::transport::TcpOutboundFailure;

pub(crate) enum ResolveTransportLeafError<E> {
    InvalidConfig(E),
    MissingLeaf,
}

pub(crate) trait ProtocolTransportLeafResolver {
    type TransportLeaf: ProtocolTransportLeaf;
    type ResolveError: std::fmt::Display;

    fn resolve_transport_leaf<'a>(
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
    PreparedTransportBridgeLeaf<<TBridge as ProtocolTransportLeafResolver>::TransportLeaf>,
    ResolveTransportLeafError<<TBridge as ProtocolTransportLeafResolver>::ResolveError>,
>
where
    TBridge: ProtocolTransportLeafResolver,
{
    bridge
        .resolve_transport_leaf(source_dir, leaf)
        .map_err(ResolveTransportLeafError::InvalidConfig)?
        .map(PreparedTransportBridgeLeaf::new)
        .ok_or(ResolveTransportLeafError::MissingLeaf)
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) fn claim_transport_bridge_tcp_leaf<'a, TBridge, TLeaf, F, E>(
    bridge: TBridge,
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
) -> Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send + Sync + 'a,
    TBridge::Opened: ProtocolTcpTransportOpenResult,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    Box::new(ClaimedTransportBridgeTcpLeaf {
        bridge,
        upstream,
        prepare_leaf,
    })
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
struct ClaimedTransportBridgeTcpLeaf<'a, TBridge, F> {
    bridge: TBridge,
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
impl<'a, TBridge, TLeaf, F, E> ClaimedTcpOutboundLeaf<'a>
    for ClaimedTransportBridgeTcpLeaf<'a, TBridge, F>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send + Sync + 'a,
    TBridge::Opened: ProtocolTcpTransportOpenResult,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    fn prepare_tcp_connect(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .map_err(|error| {
                transport_bridge_connect_claim_prepare_failure::<TBridge, _>(self.upstream, error)
            })?;
        Ok(prepare_transport_bridge_tcp_connect(&self.bridge, prepared))
    }

    fn prepare_tcp_relay_hop(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .map_err(transport_bridge_relay_claim_prepare_error::<TBridge, _>)?;
        Ok(prepare_transport_bridge_tcp_relay(&self.bridge, prepared))
    }
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) fn transport_bridge_connect_claim_prepare_failure<TBridge, E>(
    upstream: Option<(&str, u16)>,
    error: E,
) -> TcpOutboundFailure
where
    TBridge: ProtocolTcpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    TcpOutboundFailure {
        stage: TBridge::TCP_CONNECT_STAGE,
        error: invalid_input(TBridge::TCP_INVALID_CONNECT_CONFIG, error),
        upstream_endpoint: upstream.map(|(server, port)| (server.to_owned(), port)),
    }
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) fn transport_bridge_relay_claim_prepare_error<TBridge, E>(error: E) -> EngineError
where
    TBridge: ProtocolTcpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    invalid_input(TBridge::TCP_INVALID_RELAY_CONFIG, error)
}

pub(crate) fn transport_bridge_connect_prepare_failure<TBridge, E>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<E>,
) -> TcpOutboundFailure
where
    TBridge: ProtocolTcpTransportBridgeMetadata,
    E: std::fmt::Display,
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
    TcpOutboundFailure {
        stage,
        error,
        upstream_endpoint,
    }
}

pub(crate) fn transport_bridge_relay_prepare_error<TBridge, E>(
    error: ResolveTransportLeafError<E>,
) -> EngineError
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

pub(crate) fn transport_bridge_udp_direct_prepare_failure<TBridge, E>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<E>,
) -> FlowFailure
where
    TBridge: ProtocolUdpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    transport_bridge_udp_prepare_failure::<TBridge, E>(leaf, error, TBridge::UDP_DIRECT_STAGE)
}

pub(crate) fn transport_bridge_udp_relay_final_prepare_failure<TBridge, E>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<E>,
) -> FlowFailure
where
    TBridge: ProtocolUdpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    transport_bridge_udp_prepare_failure::<TBridge, E>(leaf, error, TBridge::UDP_RELAY_FINAL_STAGE)
}

pub(crate) fn transport_bridge_udp_two_stream_prepare_failure<TBridge, E>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<E>,
) -> FlowFailure
where
    TBridge: ProtocolRelayTwoStreamUdpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    transport_bridge_udp_prepare_failure::<TBridge, E>(
        leaf,
        error,
        TBridge::UDP_RELAY_CAPABILITY_STAGE,
    )
}

fn transport_bridge_udp_prepare_failure<TBridge, E>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<E>,
    stage: &'static str,
) -> FlowFailure
where
    TBridge: ProtocolUdpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    let upstream = leaf.proxy_endpoint();
    match error {
        ResolveTransportLeafError::InvalidConfig(error) => FlowFailure {
            stage,
            error: invalid_input(TBridge::UDP_INVALID_CONFIG, error),
            upstream: upstream.map(|(server, port)| (server.to_owned(), port)),
        },
        ResolveTransportLeafError::MissingLeaf => FlowFailure {
            stage,
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                TBridge::EXPECTED_OUTBOUND_LEAF,
            )),
            upstream: None,
        },
    }
}

fn invalid_input(stage: &'static str, error: impl std::fmt::Display) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
    ))
}
