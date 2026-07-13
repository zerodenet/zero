use std::future::Future;
use std::pin::Pin;

use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;
use zero_transport::managed_udp::ProtocolManagedStreamUdpBridgeOps;
use zero_transport::managed_udp::ProtocolRelayTwoStreamManagedUdpBridgeOps;
use zero_transport::outbound_leaf::{
    ProtocolRelayTwoStreamTransportLeaf, ProtocolRelayTwoStreamUdpTransportBridgeMetadata,
};
use zero_transport::outbound_leaf::{
    ProtocolTransportLeaf, ProtocolTransportLeafResolver, ProtocolUdpTransportBridgeMetadata,
};

use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::udp_dispatch::UdpDispatch;
#[cfg(feature = "socks5")]
use crate::runtime::udp_dispatch::UpstreamTrackedStart;
#[cfg(feature = "vless")]
use crate::runtime::udp_flow::managed::bridge::start_protocol_transport_bridge_udp_relay_two_stream;
use crate::runtime::udp_flow::managed::bridge::{
    start_direct_managed_stream_packet, start_protocol_transport_bridge_udp_flow,
    start_protocol_transport_bridge_udp_relay_final_hop, start_relay_managed_stream_packet,
    ManagedStreamPacketRelay, ManagedStreamPacketStartBridge,
};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::result::{FlowFailure, FlowStartResult};
use crate::runtime::Proxy;
use crate::transport::RelayCarrier;

pub(crate) trait PreparedUdpFlowOperation: Send {
    fn execute<'a>(
        self: Box<Self>,
        dispatch: &'a mut UdpDispatch,
        ctx: UdpAdapterContext<'a>,
        session: &'a Session,
        payload: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<FlowStartResult, FlowFailure>> + Send + 'a>>
    where
        Self: 'a;
}

pub(crate) struct DirectUdpFlowOperation {
    pub(crate) tag: String,
}

impl PreparedUdpFlowOperation for DirectUdpFlowOperation {
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
            execute_direct_udp_operation(
                dispatch,
                ctx.proxy(),
                session,
                payload,
                PreparedDirectUdpOperation { tag: &self.tag },
            )
            .await
        })
    }
}

pub(crate) struct ManagedDatagramUdpOperation<'a, T> {
    pub(crate) plan: zero_transport::managed_udp::ManagedDatagramStartPlan<'a, T>,
    pub(crate) needs_proxy: bool,
}

impl<T> PreparedUdpFlowOperation for ManagedDatagramUdpOperation<'_, T>
where
    T: std::any::Any + Send + Sync + std::fmt::Debug,
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
            let proxy = self.needs_proxy.then(|| ctx.proxy());
            execute_managed_datagram_operation(dispatch, proxy, session, payload, self.plan).await
        })
    }
}

#[cfg(feature = "socks5")]
pub(crate) struct RegisteredAssociationUdpOperation<'a, T> {
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: T,
}

#[cfg(feature = "socks5")]
impl<T> PreparedUdpFlowOperation for RegisteredAssociationUdpOperation<'_, T>
where
    T: std::any::Any + Send + Sync + std::fmt::Debug,
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
            execute_registered_association_operation(
                dispatch,
                session,
                payload,
                PreparedRegisteredAssociationOperation {
                    proxy: Some(ctx.proxy()),
                    tag: self.tag,
                    server: self.server,
                    port: self.port,
                    resume: self.resume,
                },
            )
            .await
        })
    }
}

pub(crate) struct ManagedStreamPacketUdpOperation<'a, T> {
    pub(crate) operation: PreparedManagedStreamPacketOperation<'a, T>,
    pub(crate) needs_proxy: bool,
}

impl<T> PreparedUdpFlowOperation for ManagedStreamPacketUdpOperation<'_, T>
where
    T: std::any::Any + Send + Sync + std::fmt::Debug,
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
            let proxy = self.needs_proxy.then(|| ctx.proxy());
            execute_managed_stream_packet_operation(
                dispatch,
                proxy,
                session,
                payload,
                self.operation,
            )
            .await
        })
    }
}

pub(crate) struct PreparedDirectUdpOperation<'a> {
    pub(crate) tag: &'a str,
}

pub(crate) async fn execute_direct_udp_operation(
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    payload: &[u8],
    operation: PreparedDirectUdpOperation<'_>,
) -> Result<FlowStartResult, FlowFailure> {
    let target_addr = proxy
        .protocols
        .direct_connector()
        .resolve_target_addr(session, proxy.resolver.as_ref())
        .await
        .map_err(|error| FlowFailure {
            stage: "resolve_udp_target",
            error: error.into(),
            upstream: None,
        })?;
    let sent = dispatch
        .send_direct_packet(target_addr, payload)
        .await
        .map_err(|error| FlowFailure {
            stage: "udp_direct_send",
            error,
            upstream: None,
        })?;
    Ok(FlowStartResult::Flow {
        outbound: Box::new(UdpFlowOutbound::Direct {
            tag: operation.tag.to_owned(),
            target_addr,
        }),
        tx_bytes: sent as u64,
    })
}

pub(crate) async fn execute_managed_datagram_operation<T>(
    dispatch: &mut UdpDispatch,
    proxy: Option<&Proxy>,
    session: &Session,
    payload: &[u8],
    operation: zero_transport::managed_udp::ManagedDatagramStartPlan<'_, T>,
) -> Result<FlowStartResult, FlowFailure>
where
    T: std::any::Any + Send + Sync + std::fmt::Debug,
{
    dispatch
        .start_transport_managed_datagram(proxy, session, payload, operation)
        .await
}

#[cfg(feature = "socks5")]
pub(crate) struct PreparedRegisteredAssociationOperation<'a, T> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: T,
}

#[cfg(feature = "socks5")]
pub(crate) async fn execute_registered_association_operation<T>(
    dispatch: &mut UdpDispatch,
    session: &Session,
    payload: &[u8],
    operation: PreparedRegisteredAssociationOperation<'_, T>,
) -> Result<FlowStartResult, FlowFailure>
where
    T: std::any::Any + Send + Sync + std::fmt::Debug,
{
    dispatch
        .start_tracked_upstream(UpstreamTrackedStart {
            proxy: operation.proxy,
            tag: operation.tag,
            session,
            server: operation.server,
            port: operation.port,
            resume: operation.resume,
            payload,
        })
        .await
}

pub(crate) enum PreparedTransportUdpOperation<'a, 'leaf> {
    Direct {
        leaf: &'leaf ResolvedLeafOutbound<'a>,
    },
    RelayFinalHop {
        carrier: RelayCarrier,
        leaf: &'leaf ResolvedLeafOutbound<'a>,
    },
}

pub(crate) enum PreparedManagedStreamPacketOperation<'a, T> {
    Direct {
        plan: zero_transport::managed_udp::ManagedStreamPacketBridgePlan<'a, T>,
    },
    RelayFinalHop {
        plan: zero_transport::managed_udp::ManagedStreamPacketBridgePlan<'a, T>,
        carrier: RelayCarrier,
    },
}

pub(crate) struct TransportBridgeUdpOperation<'a, TBridge> {
    pub(crate) bridge: &'a TBridge,
    pub(crate) operation: PreparedTransportUdpOperation<'a, 'a>,
}

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

pub(crate) struct RelayTwoStreamUdpOperation<'a, TBridge> {
    pub(crate) bridge: &'a TBridge,
    pub(crate) chain: Vec<ResolvedLeafOutbound<'a>>,
}

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
    start_protocol_transport_bridge_udp_relay_two_stream(
        bridge,
        dispatch.flow_start_context(),
        proxy,
        session,
        &operation.chain,
        payload,
    )
    .await
}

pub(crate) async fn execute_managed_stream_packet_operation<T>(
    dispatch: &mut UdpDispatch,
    proxy: Option<&Proxy>,
    session: &Session,
    payload: &[u8],
    operation: PreparedManagedStreamPacketOperation<'_, T>,
) -> Result<FlowStartResult, FlowFailure>
where
    T: std::any::Any + Send + Sync + std::fmt::Debug,
{
    let mut context = dispatch.flow_start_context();
    match operation {
        PreparedManagedStreamPacketOperation::Direct { plan } => {
            debug_assert!(!plan.relay_chain);
            let proxy = proxy.expect("direct managed stream operation requires proxy context");
            start_direct_managed_stream_packet(
                &mut context,
                ManagedStreamPacketStartBridge::direct(
                    proxy,
                    plan.tag,
                    session,
                    (plan.server, plan.port),
                    plan.resume,
                    payload,
                ),
            )
            .await
        }
        PreparedManagedStreamPacketOperation::RelayFinalHop { plan, carrier } => {
            debug_assert!(plan.relay_chain);
            start_relay_managed_stream_packet(
                &mut context,
                ManagedStreamPacketStartBridge::relay(
                    proxy,
                    plan.tag,
                    session,
                    ManagedStreamPacketRelay {
                        carrier,
                        tls_server_name: None,
                    },
                    (plan.server, plan.port),
                    plan.resume,
                    payload,
                ),
            )
            .await
        }
    }
}

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
            start_protocol_transport_bridge_udp_flow(
                bridge,
                dispatch.flow_start_context(),
                proxy,
                session,
                leaf,
                payload,
            )
            .await
        }
        PreparedTransportUdpOperation::RelayFinalHop { carrier, leaf } => {
            start_protocol_transport_bridge_udp_relay_final_hop(
                bridge,
                dispatch.flow_start_context(),
                proxy,
                session,
                carrier,
                leaf,
                payload,
            )
            .await
        }
    }
}
