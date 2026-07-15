use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use zero_core::Session;
use zero_engine::EngineError;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use zero_transport::managed_udp::ProtocolManagedStreamUdpBridgeOps;
#[cfg(feature = "vless")]
use zero_transport::managed_udp::ProtocolRelayTwoStreamManagedUdpBridgeOps;
use zero_transport::outbound_leaf::PreparedTransportBridgeLeaf;
#[cfg(feature = "vless")]
use zero_transport::outbound_leaf::{
    open_prepared_relay_two_stream_udp_transport, prepared_relay_two_stream_udp_resume,
    prepared_udp_relay_needs_two_streams, ProtocolRelayTwoStreamTransportLeaf,
    ProtocolRelayTwoStreamUdpTransportBridgeMetadata,
};
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use zero_transport::outbound_leaf::{
    prepared_direct_udp_resume, prepared_relay_final_hop_udp_resume, ProtocolTransportLeaf,
    ProtocolUdpTransportBridgeMetadata,
};

use super::super::{ClaimedUdpFlowLeaf, UdpAdapterContext};
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::bridge::{
    start_direct_managed_stream_packet, start_relay_managed_stream_packet,
    ManagedStreamPacketRelay, ManagedStreamPacketStartBridge,
};
use crate::transport::RelayCarrier;

pub(crate) fn claim_transport_bridge_udp_leaf<'a, TBridge, TLeaf, F, E>(
    bridge: TBridge,
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
) -> Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolUdpTransportBridgeMetadata
        + ProtocolManagedStreamUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send + 'a,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    Box::new(ClaimedTransportBridgeUdpLeaf {
        bridge,
        upstream,
        prepare_leaf,
    })
}

#[cfg(feature = "vless")]
pub(crate) fn claim_relay_two_stream_transport_bridge_udp_leaf<'a, TBridge, TLeaf, F, E>(
    bridge: TBridge,
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
) -> Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolUdpTransportBridgeMetadata
        + ProtocolManagedStreamUdpBridgeOps<TLeaf>
        + ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolRelayTwoStreamTransportLeaf + Send + Sync + 'a,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    Box::new(ClaimedRelayTwoStreamTransportBridgeUdpLeaf {
        bridge,
        upstream,
        prepare_leaf,
    })
}

enum PreparedTransportUdpOperation<TLeaf> {
    Direct {
        prepared: PreparedTransportBridgeLeaf<TLeaf>,
    },
    RelayFinalHop {
        carrier: RelayCarrier,
        prepared: PreparedTransportBridgeLeaf<TLeaf>,
    },
}

struct TransportBridgeUdpOperation<TBridge, TLeaf> {
    bridge: TBridge,
    operation: PreparedTransportUdpOperation<TLeaf>,
}

struct RelayTwoStreamUdpOperation<TBridge, TLeaf> {
    bridge: TBridge,
    post_carrier: RelayCarrier,
    get_carrier: RelayCarrier,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
}

struct ClaimedTransportBridgeUdpLeaf<'a, TBridge, F> {
    bridge: TBridge,
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
}

#[cfg(feature = "vless")]
struct ClaimedRelayTwoStreamTransportBridgeUdpLeaf<'a, TBridge, F> {
    bridge: TBridge,
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) fn prepare_transport_bridge_udp_direct<'a, TBridge, TLeaf>(
    bridge: &TBridge,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
) -> Box<dyn PreparedUdpFlowOperation + 'a>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolUdpTransportBridgeMetadata
        + ProtocolManagedStreamUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send + 'a,
{
    Box::new(TransportBridgeUdpOperation::<TBridge, TLeaf> {
        bridge: bridge.clone(),
        operation: PreparedTransportUdpOperation::Direct { prepared },
    })
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) fn prepare_owned_transport_bridge_udp_relay_final_hop<'a, TBridge, TLeaf>(
    bridge: &TBridge,
    carrier: RelayCarrier,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
) -> Box<dyn PreparedUdpFlowOperation + 'a>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolUdpTransportBridgeMetadata
        + ProtocolManagedStreamUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send + 'a,
{
    Box::new(TransportBridgeUdpOperation::<TBridge, TLeaf> {
        bridge: bridge.clone(),
        operation: PreparedTransportUdpOperation::RelayFinalHop { carrier, prepared },
    })
}

#[cfg(feature = "vless")]
pub(crate) fn prepare_owned_transport_bridge_udp_relay_two_stream<'a, TBridge, TLeaf>(
    bridge: &TBridge,
    post_carrier: RelayCarrier,
    get_carrier: RelayCarrier,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
) -> Box<dyn PreparedUdpFlowOperation + 'a>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolRelayTwoStreamTransportLeaf + Send + Sync + 'a,
{
    Box::new(RelayTwoStreamUdpOperation::<TBridge, TLeaf> {
        bridge: bridge.clone(),
        post_carrier,
        get_carrier,
        prepared,
    })
}

#[cfg(feature = "vless")]
pub(crate) fn transport_bridge_udp_relay_needs_two_streams<TBridge, TLeaf>(
    bridge: &TBridge,
    prepared: &PreparedTransportBridgeLeaf<TLeaf>,
) -> bool
where
    TBridge: ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolRelayTwoStreamTransportLeaf,
{
    prepared_udp_relay_needs_two_streams(bridge, prepared)
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
impl<'a, TBridge, TLeaf, F, E> ClaimedUdpFlowLeaf<'a>
    for ClaimedTransportBridgeUdpLeaf<'a, TBridge, F>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolUdpTransportBridgeMetadata
        + ProtocolManagedStreamUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send + 'a,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    fn prepare_udp_flow(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .map_err(|error| {
                transport_bridge_udp_direct_claim_prepare_failure::<TBridge, _>(
                    self.upstream,
                    error,
                )
            })?;
        Ok(prepare_transport_bridge_udp_direct(&self.bridge, prepared))
    }

    fn prepare_owned_udp_relay_final_hop(
        &self,
        carrier: RelayCarrier,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .map_err(|error| {
                transport_bridge_udp_relay_final_claim_prepare_failure::<TBridge, _>(
                    self.upstream,
                    error,
                )
            })?;
        Ok(prepare_owned_transport_bridge_udp_relay_final_hop(
            &self.bridge,
            carrier,
            prepared,
        ))
    }
}

#[cfg(feature = "vless")]
impl<'a, TBridge, TLeaf, F, E> ClaimedUdpFlowLeaf<'a>
    for ClaimedRelayTwoStreamTransportBridgeUdpLeaf<'a, TBridge, F>
where
    TBridge: Send
        + Sync
        + Clone
        + 'a
        + ProtocolUdpTransportBridgeMetadata
        + ProtocolManagedStreamUdpBridgeOps<TLeaf>
        + ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolRelayTwoStreamTransportLeaf + Send + Sync + 'a,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    fn prepare_udp_flow(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .map_err(|error| {
                transport_bridge_udp_direct_claim_prepare_failure::<TBridge, _>(
                    self.upstream,
                    error,
                )
            })?;
        Ok(prepare_transport_bridge_udp_direct(&self.bridge, prepared))
    }

    fn udp_relay_needs_two_streams(&self, source_dir: Option<&Path>) -> bool {
        (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .is_ok_and(|prepared| {
                transport_bridge_udp_relay_needs_two_streams(&self.bridge, &prepared)
            })
    }

    fn prepare_owned_udp_relay_final_hop(
        &self,
        carrier: RelayCarrier,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .map_err(|error| {
                transport_bridge_udp_relay_final_claim_prepare_failure::<TBridge, _>(
                    self.upstream,
                    error,
                )
            })?;
        Ok(prepare_owned_transport_bridge_udp_relay_final_hop(
            &self.bridge,
            carrier,
            prepared,
        ))
    }

    fn prepare_owned_udp_relay_two_stream(
        &self,
        post_carrier: RelayCarrier,
        get_carrier: RelayCarrier,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportBridgeLeaf::new)
            .map_err(|error| {
                transport_bridge_udp_two_stream_claim_prepare_failure::<TBridge, _>(
                    self.upstream,
                    error,
                )
            })?;
        Ok(prepare_owned_transport_bridge_udp_relay_two_stream(
            &self.bridge,
            post_carrier,
            get_carrier,
            prepared,
        ))
    }
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
impl<TBridge, TLeaf> PreparedUdpFlowOperation for TransportBridgeUdpOperation<TBridge, TLeaf>
where
    TBridge:
        Send + Sync + ProtocolUdpTransportBridgeMetadata + ProtocolManagedStreamUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send,
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
                &self.bridge,
                dispatch,
                ctx.runtime_services(),
                session,
                payload,
                self.operation,
            )
            .await
        })
    }
}

#[cfg(feature = "vless")]
impl<TBridge, TLeaf> PreparedUdpFlowOperation for RelayTwoStreamUdpOperation<TBridge, TLeaf>
where
    TBridge: Send
        + Sync
        + ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolRelayTwoStreamTransportLeaf + Send + Sync,
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
                &self.bridge,
                dispatch,
                ctx.runtime_services(),
                session,
                payload,
                self.post_carrier,
                self.get_carrier,
                self.prepared,
            )
            .await
        })
    }
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

fn transport_bridge_udp_direct_claim_prepare_failure<TBridge, E>(
    upstream: Option<(&str, u16)>,
    error: E,
) -> FlowFailure
where
    TBridge: ProtocolUdpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    transport_bridge_udp_claim_prepare_failure::<TBridge, E>(
        upstream,
        error,
        TBridge::UDP_DIRECT_STAGE,
    )
}

fn transport_bridge_udp_relay_final_claim_prepare_failure<TBridge, E>(
    upstream: Option<(&str, u16)>,
    error: E,
) -> FlowFailure
where
    TBridge: ProtocolUdpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    transport_bridge_udp_claim_prepare_failure::<TBridge, E>(
        upstream,
        error,
        TBridge::UDP_RELAY_FINAL_STAGE,
    )
}

#[cfg(feature = "vless")]
fn transport_bridge_udp_two_stream_claim_prepare_failure<TBridge, E>(
    upstream: Option<(&str, u16)>,
    error: E,
) -> FlowFailure
where
    TBridge: ProtocolRelayTwoStreamUdpTransportBridgeMetadata + ProtocolUdpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    transport_bridge_udp_claim_prepare_failure::<TBridge, E>(
        upstream,
        error,
        TBridge::UDP_RELAY_CAPABILITY_STAGE,
    )
}

fn transport_bridge_udp_claim_prepare_failure<TBridge, E>(
    upstream: Option<(&str, u16)>,
    error: E,
    stage: &'static str,
) -> FlowFailure
where
    TBridge: ProtocolUdpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    FlowFailure {
        stage,
        error: invalid_input(TBridge::UDP_INVALID_CONFIG, error),
        upstream: upstream.map(|(server, port)| (server.to_owned(), port)),
    }
}

fn invalid_input(stage: &'static str, error: impl std::fmt::Display) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
    ))
}

#[cfg(feature = "vless")]
async fn execute_relay_two_stream_udp_operation<TBridge, TLeaf>(
    bridge: &TBridge,
    dispatch: &mut UdpDispatch,
    services: crate::protocol_registry::UdpRuntimeServices,
    session: &Session,
    payload: &[u8],
    post_carrier: RelayCarrier,
    get_carrier: RelayCarrier,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
) -> Result<FlowStartResult, FlowFailure>
where
    TBridge: ProtocolRelayTwoStreamUdpTransportBridgeMetadata
        + ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolRelayTwoStreamTransportLeaf,
{
    let mut context = dispatch.flow_start_context();
    let endpoint = prepared.endpoint();
    let resume = prepared_relay_two_stream_udp_resume(bridge, &prepared);
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
            Some(services),
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
async fn execute_transport_udp_operation<TBridge, TLeaf>(
    bridge: &TBridge,
    dispatch: &mut UdpDispatch,
    services: crate::protocol_registry::UdpRuntimeServices,
    session: &Session,
    payload: &[u8],
    operation: PreparedTransportUdpOperation<TLeaf>,
) -> Result<FlowStartResult, FlowFailure>
where
    TBridge: ProtocolUdpTransportBridgeMetadata + ProtocolManagedStreamUdpBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf,
{
    match operation {
        PreparedTransportUdpOperation::Direct { prepared } => {
            let mut context = dispatch.flow_start_context();
            let endpoint = prepared.endpoint();
            start_direct_managed_stream_packet(
                &mut context,
                ManagedStreamPacketStartBridge::direct(
                    services,
                    endpoint.tag,
                    session,
                    (endpoint.server, endpoint.port),
                    prepared_direct_udp_resume(bridge, &prepared),
                    payload,
                ),
            )
            .await
        }
        PreparedTransportUdpOperation::RelayFinalHop { carrier, prepared } => {
            let mut context = dispatch.flow_start_context();
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
                    Some(services),
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
