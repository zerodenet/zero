use std::future::Future;
use std::pin::Pin;

use zero_core::Session;

use crate::protocol_registry::UdpAdapterContext;
use crate::runtime::udp_dispatch::UdpDispatch;
#[cfg(feature = "socks5")]
use crate::runtime::udp_dispatch::UpstreamTrackedStart;
#[cfg(feature = "mieru")]
use crate::runtime::udp_flow::managed::bridge::{
    start_direct_managed_stream_packet, start_relay_managed_stream_packet,
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

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) struct ManagedDatagramUdpOperation<'a, T> {
    pub(crate) plan: zero_transport::managed_udp::ManagedDatagramStartPlan<'a, T>,
    pub(crate) needs_proxy: bool,
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
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

#[cfg(feature = "mieru")]
pub(crate) struct ManagedStreamPacketUdpOperation<'a, T> {
    pub(crate) operation: PreparedManagedStreamPacketOperation<'a, T>,
    pub(crate) needs_proxy: bool,
}

#[cfg(feature = "mieru")]
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

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
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

#[cfg(feature = "mieru")]
pub(crate) enum PreparedManagedStreamPacketOperation<'a, T> {
    Direct {
        plan: zero_transport::managed_udp::ManagedStreamPacketBridgePlan<'a, T>,
    },
    RelayFinalHop {
        plan: zero_transport::managed_udp::ManagedStreamPacketBridgePlan<'a, T>,
        carrier: RelayCarrier,
    },
}

#[cfg(feature = "mieru")]
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
