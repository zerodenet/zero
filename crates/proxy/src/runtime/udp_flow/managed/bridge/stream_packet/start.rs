use std::any::Any;

use zero_core::Session;

use super::request::ManagedStreamPacketStartBridge;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedStreamPacketStart, UdpDispatch,
};
use crate::runtime::Proxy;
use crate::transport::RelayCarrier;

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
