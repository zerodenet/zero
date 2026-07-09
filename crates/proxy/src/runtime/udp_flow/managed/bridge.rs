use std::any::Any;

use zero_core::Session;
use zero_transport::managed_udp::ProtocolManagedStreamFlowStages;

use super::model::ManagedStreamFlowHandler;
use super::stream_manager::{ManagedStreamFlowConnector, ManagedStreamFlowManager};
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedStreamPacketStart, UdpDispatch,
};
use crate::runtime::Proxy;
use crate::transport::RelayCarrier;

pub(crate) type ManagedStreamStages = ProtocolManagedStreamFlowStages;

pub(crate) fn managed_stream_handler_box<T>(
    stages: ManagedStreamStages,
) -> Box<dyn ManagedStreamFlowHandler>
where
    T: ManagedStreamFlowConnector,
{
    Box::new(ManagedStreamFlowManager::<T>::new(
        stages.establish_stage,
        stages.relay_upstream_stage,
        stages.relay_establish_stage,
        stages.relay_send_stage,
        stages.mismatch_stage,
        stages.mismatch_message,
    ))
}

struct ManagedStreamPacketStartBridge<'a, T> {
    proxy: Option<&'a Proxy>,
    tag: &'a str,
    session: &'a Session,
    carrier: Option<RelayCarrier>,
    tls_server_name: Option<&'a str>,
    server: &'a str,
    port: u16,
    resume: T,
    payload: &'a [u8],
    relay_chain: bool,
}

impl<'a, T> ManagedStreamPacketStartBridge<'a, T> {
    fn direct(
        proxy: &'a Proxy,
        tag: &'a str,
        session: &'a Session,
        server: &'a str,
        port: u16,
        resume: T,
        payload: &'a [u8],
    ) -> Self {
        Self {
            proxy: Some(proxy),
            tag,
            session,
            carrier: None,
            tls_server_name: None,
            server,
            port,
            resume,
            payload,
            relay_chain: false,
        }
    }

    fn relay(
        proxy: Option<&'a Proxy>,
        tag: &'a str,
        session: &'a Session,
        carrier: RelayCarrier,
        tls_server_name: Option<&'a str>,
        server: &'a str,
        port: u16,
        resume: T,
        payload: &'a [u8],
    ) -> Self {
        Self {
            proxy,
            tag,
            session,
            carrier: Some(carrier),
            tls_server_name,
            server,
            port,
            resume,
            payload,
            relay_chain: true,
        }
    }
}

async fn start_managed_stream_packet<T>(
    dispatch: &mut UdpDispatch,
    request: ManagedStreamPacketStartBridge<'_, T>,
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    dispatch
        .start_tracked_managed_stream_packet(ManagedStreamPacketStart {
            proxy: request.proxy,
            tag: request.tag,
            session: request.session,
            carrier: request.carrier,
            tls_server_name: request.tls_server_name,
            server: request.server,
            port: request.port,
            resume: request.resume,
            payload: request.payload,
            relay_chain: request.relay_chain,
        })
        .await
}

pub(crate) async fn start_direct_managed_stream_packet<T>(
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    tag: &str,
    session: &Session,
    server: &str,
    port: u16,
    resume: T,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    start_managed_stream_packet(
        dispatch,
        ManagedStreamPacketStartBridge::direct(proxy, tag, session, server, port, resume, payload),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_relay_managed_stream_packet<T>(
    dispatch: &mut UdpDispatch,
    proxy: Option<&Proxy>,
    tag: &str,
    session: &Session,
    carrier: RelayCarrier,
    tls_server_name: Option<&str>,
    server: &str,
    port: u16,
    resume: T,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure>
where
    T: Any + Send + Sync + std::fmt::Debug,
{
    start_managed_stream_packet(
        dispatch,
        ManagedStreamPacketStartBridge::relay(
            proxy,
            tag,
            session,
            carrier,
            tls_server_name,
            server,
            port,
            resume,
            payload,
        ),
    )
    .await
}
