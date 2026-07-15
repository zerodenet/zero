use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use zero_core::Session;
use zero_engine::EngineError;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use zero_transport::managed_udp::ProtocolManagedStreamUdpLeafOps;
#[cfg(feature = "vless")]
use zero_transport::managed_udp::ProtocolRelayTwoStreamManagedUdpLeafOps;
use zero_transport::outbound_leaf::PreparedTransportLeaf;
#[cfg(feature = "vless")]
use zero_transport::outbound_leaf::{
    open_prepared_relay_two_stream_udp_transport, prepared_relay_two_stream_udp_resume,
    prepared_udp_relay_needs_two_streams, ProtocolRelayTwoStreamTransportLeaf,
    ProtocolRelayTwoStreamUdpTransportLeafMetadata,
};
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
use zero_transport::outbound_leaf::{
    prepared_direct_udp_resume, prepared_relay_final_hop_udp_resume, ProtocolTransportLeaf,
    ProtocolUdpTransportLeafMetadata,
};

use super::super::{ClaimedUdpFlowLeaf, UdpAdapterContext};
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::bridge::{
    start_direct_managed_stream_packet, start_relay_managed_stream_packet,
    ManagedStreamPacketRelay, ManagedStreamPacketStartBridge,
};
use crate::transport::RelayCarrier;

pub(crate) fn claim_transport_udp_leaf<'a, TLeaf, F, E>(
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
) -> Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>
where
    TLeaf: ProtocolTransportLeaf
        + ProtocolUdpTransportLeafMetadata
        + ProtocolManagedStreamUdpLeafOps
        + Send
        + 'a,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    Box::new(ClaimedTransportUdpLeaf {
        upstream,
        prepare_leaf,
    })
}

#[cfg(feature = "vless")]
pub(crate) fn claim_relay_two_stream_transport_udp_leaf<'a, TLeaf, F, E>(
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
) -> Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf
        + ProtocolRelayTwoStreamUdpTransportLeafMetadata
        + ProtocolManagedStreamUdpLeafOps
        + ProtocolRelayTwoStreamManagedUdpLeafOps
        + Send
        + Sync
        + 'a,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    Box::new(ClaimedRelayTwoStreamTransportUdpLeaf {
        upstream,
        prepare_leaf,
    })
}

enum PreparedTransportUdpOperation<TLeaf> {
    Direct {
        prepared: PreparedTransportLeaf<TLeaf>,
    },
    RelayFinalHop {
        carrier: RelayCarrier,
        prepared: PreparedTransportLeaf<TLeaf>,
    },
}

struct TransportLeafUdpOperation<TLeaf> {
    operation: PreparedTransportUdpOperation<TLeaf>,
}

struct RelayTwoStreamUdpOperation<TLeaf> {
    post_carrier: RelayCarrier,
    get_carrier: RelayCarrier,
    prepared: PreparedTransportLeaf<TLeaf>,
}

struct ClaimedTransportUdpLeaf<'a, F> {
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
}

#[cfg(feature = "vless")]
struct ClaimedRelayTwoStreamTransportUdpLeaf<'a, F> {
    upstream: Option<(&'a str, u16)>,
    prepare_leaf: F,
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) fn prepare_transport_udp_direct<'a, TLeaf>(
    prepared: PreparedTransportLeaf<TLeaf>,
) -> Box<dyn PreparedUdpFlowOperation + 'a>
where
    TLeaf: ProtocolTransportLeaf
        + ProtocolUdpTransportLeafMetadata
        + ProtocolManagedStreamUdpLeafOps
        + Send
        + 'a,
{
    Box::new(TransportLeafUdpOperation::<TLeaf> {
        operation: PreparedTransportUdpOperation::Direct { prepared },
    })
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) fn prepare_owned_transport_udp_relay_final_hop<'a, TLeaf>(
    carrier: RelayCarrier,
    prepared: PreparedTransportLeaf<TLeaf>,
) -> Box<dyn PreparedUdpFlowOperation + 'a>
where
    TLeaf: ProtocolTransportLeaf
        + ProtocolUdpTransportLeafMetadata
        + ProtocolManagedStreamUdpLeafOps
        + Send
        + 'a,
{
    Box::new(TransportLeafUdpOperation::<TLeaf> {
        operation: PreparedTransportUdpOperation::RelayFinalHop { carrier, prepared },
    })
}

#[cfg(feature = "vless")]
pub(crate) fn prepare_owned_transport_udp_relay_two_stream<'a, TLeaf>(
    post_carrier: RelayCarrier,
    get_carrier: RelayCarrier,
    prepared: PreparedTransportLeaf<TLeaf>,
) -> Box<dyn PreparedUdpFlowOperation + 'a>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf
        + ProtocolRelayTwoStreamUdpTransportLeafMetadata
        + ProtocolRelayTwoStreamManagedUdpLeafOps
        + Send
        + Sync
        + 'a,
{
    Box::new(RelayTwoStreamUdpOperation::<TLeaf> {
        post_carrier,
        get_carrier,
        prepared,
    })
}

#[cfg(feature = "vless")]
pub(crate) fn transport_udp_relay_needs_two_streams<TLeaf>(
    prepared: &PreparedTransportLeaf<TLeaf>,
) -> bool
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf
        + ProtocolRelayTwoStreamUdpTransportLeafMetadata
        + ProtocolRelayTwoStreamManagedUdpLeafOps,
{
    prepared_udp_relay_needs_two_streams(prepared)
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
impl<'a, TLeaf, F, E> ClaimedUdpFlowLeaf<'a> for ClaimedTransportUdpLeaf<'a, F>
where
    TLeaf: ProtocolTransportLeaf
        + ProtocolUdpTransportLeafMetadata
        + ProtocolManagedStreamUdpLeafOps
        + Send
        + 'a,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    fn prepare_udp_flow(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportLeaf::new)
            .map_err(|error| {
                transport_udp_direct_claim_prepare_failure::<TLeaf, _>(self.upstream, error)
            })?;
        Ok(prepare_transport_udp_direct(prepared))
    }

    fn prepare_owned_udp_relay_final_hop(
        &self,
        carrier: RelayCarrier,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportLeaf::new)
            .map_err(|error| {
                transport_udp_relay_final_claim_prepare_failure::<TLeaf, _>(self.upstream, error)
            })?;
        Ok(prepare_owned_transport_udp_relay_final_hop(
            carrier, prepared,
        ))
    }
}

#[cfg(feature = "vless")]
impl<'a, TLeaf, F, E> ClaimedUdpFlowLeaf<'a> for ClaimedRelayTwoStreamTransportUdpLeaf<'a, F>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf
        + ProtocolRelayTwoStreamUdpTransportLeafMetadata
        + ProtocolManagedStreamUdpLeafOps
        + ProtocolRelayTwoStreamManagedUdpLeafOps
        + Send
        + Sync
        + 'a,
    F: Fn(Option<&Path>) -> Result<TLeaf, E> + Send + Sync + 'a,
    E: std::fmt::Display,
{
    fn prepare_udp_flow(
        &self,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportLeaf::new)
            .map_err(|error| {
                transport_udp_direct_claim_prepare_failure::<TLeaf, _>(self.upstream, error)
            })?;
        Ok(prepare_transport_udp_direct(prepared))
    }

    fn udp_relay_needs_two_streams(&self, source_dir: Option<&Path>) -> bool {
        (self.prepare_leaf)(source_dir)
            .map(PreparedTransportLeaf::new)
            .is_ok_and(|prepared| transport_udp_relay_needs_two_streams(&prepared))
    }

    fn prepare_owned_udp_relay_final_hop(
        &self,
        carrier: RelayCarrier,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportLeaf::new)
            .map_err(|error| {
                transport_udp_relay_final_claim_prepare_failure::<TLeaf, _>(self.upstream, error)
            })?;
        Ok(prepare_owned_transport_udp_relay_final_hop(
            carrier, prepared,
        ))
    }

    fn prepare_owned_udp_relay_two_stream(
        &self,
        post_carrier: RelayCarrier,
        get_carrier: RelayCarrier,
        source_dir: Option<&Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared = (self.prepare_leaf)(source_dir)
            .map(PreparedTransportLeaf::new)
            .map_err(|error| {
                transport_udp_two_stream_claim_prepare_failure::<TLeaf, _>(self.upstream, error)
            })?;
        Ok(prepare_owned_transport_udp_relay_two_stream(
            post_carrier,
            get_carrier,
            prepared,
        ))
    }
}

#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
impl<TLeaf> PreparedUdpFlowOperation for TransportLeafUdpOperation<TLeaf>
where
    TLeaf: ProtocolTransportLeaf
        + ProtocolUdpTransportLeafMetadata
        + ProtocolManagedStreamUdpLeafOps
        + Send,
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
impl<TLeaf> PreparedUdpFlowOperation for RelayTwoStreamUdpOperation<TLeaf>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf
        + ProtocolRelayTwoStreamUdpTransportLeafMetadata
        + ProtocolRelayTwoStreamManagedUdpLeafOps
        + Send
        + Sync,
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

fn transport_udp_direct_claim_prepare_failure<TLeaf, E>(
    upstream: Option<(&str, u16)>,
    error: E,
) -> FlowFailure
where
    TLeaf: ProtocolUdpTransportLeafMetadata,
    E: std::fmt::Display,
{
    transport_udp_claim_prepare_failure::<TLeaf, E>(upstream, error, TLeaf::UDP_DIRECT_STAGE)
}

fn transport_udp_relay_final_claim_prepare_failure<TLeaf, E>(
    upstream: Option<(&str, u16)>,
    error: E,
) -> FlowFailure
where
    TLeaf: ProtocolUdpTransportLeafMetadata,
    E: std::fmt::Display,
{
    transport_udp_claim_prepare_failure::<TLeaf, E>(upstream, error, TLeaf::UDP_RELAY_FINAL_STAGE)
}

#[cfg(feature = "vless")]
fn transport_udp_two_stream_claim_prepare_failure<TLeaf, E>(
    upstream: Option<(&str, u16)>,
    error: E,
) -> FlowFailure
where
    TLeaf: ProtocolRelayTwoStreamUdpTransportLeafMetadata,
    E: std::fmt::Display,
{
    transport_udp_claim_prepare_failure::<TLeaf, E>(
        upstream,
        error,
        TLeaf::UDP_RELAY_CAPABILITY_STAGE,
    )
}

fn transport_udp_claim_prepare_failure<TLeaf, E>(
    upstream: Option<(&str, u16)>,
    error: E,
    stage: &'static str,
) -> FlowFailure
where
    TLeaf: ProtocolUdpTransportLeafMetadata,
    E: std::fmt::Display,
{
    FlowFailure {
        stage,
        error: invalid_input(TLeaf::UDP_INVALID_CONFIG, error),
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
async fn execute_relay_two_stream_udp_operation<TLeaf>(
    dispatch: &mut UdpDispatch,
    services: crate::protocol_registry::UdpRuntimeServices,
    session: &Session,
    payload: &[u8],
    post_carrier: RelayCarrier,
    get_carrier: RelayCarrier,
    prepared: PreparedTransportLeaf<TLeaf>,
) -> Result<FlowStartResult, FlowFailure>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf
        + ProtocolRelayTwoStreamUdpTransportLeafMetadata
        + ProtocolRelayTwoStreamManagedUdpLeafOps,
{
    let mut context = dispatch.flow_start_context();
    let endpoint = prepared.endpoint();
    let resume = prepared_relay_two_stream_udp_resume(&prepared);
    let paired_stream = open_prepared_relay_two_stream_udp_transport(
        &prepared,
        post_carrier.stream,
        get_carrier.stream,
    )
    .await
    .map_err(|error| FlowFailure {
        stage: TLeaf::UDP_RELAY_CHAIN_STAGE,
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
async fn execute_transport_udp_operation<TLeaf>(
    dispatch: &mut UdpDispatch,
    services: crate::protocol_registry::UdpRuntimeServices,
    session: &Session,
    payload: &[u8],
    operation: PreparedTransportUdpOperation<TLeaf>,
) -> Result<FlowStartResult, FlowFailure>
where
    TLeaf:
        ProtocolTransportLeaf + ProtocolUdpTransportLeafMetadata + ProtocolManagedStreamUdpLeafOps,
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
                    prepared_direct_udp_resume(&prepared),
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
                    prepared_relay_final_hop_udp_resume(&prepared),
                    payload,
                ),
            )
            .await
        }
    }
}
