use zero_core::Session;
use zero_transport::mieru_transport::MieruManagedUdpFlowPlan;

use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::bridge::{
    start_direct_managed_stream_packet, start_relay_managed_stream_packet,
    ManagedStreamPacketRelay, ManagedStreamPacketStartBridge,
};
use crate::runtime::Proxy;

pub(super) async fn start(
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    payload: &[u8],
    plan: MieruManagedUdpFlowPlan<'_>,
) -> Result<FlowStartResult, FlowFailure> {
    let (tag, server, port, resume) = plan.into_parts();
    let mut context = dispatch.flow_start_context();
    start_direct_managed_stream_packet(
        &mut context,
        ManagedStreamPacketStartBridge::direct(
            proxy,
            tag,
            session,
            (server, port),
            resume,
            payload,
        ),
    )
    .await
}

pub(super) async fn start_relay_final_hop(
    dispatch: &mut UdpDispatch,
    session: &Session,
    carrier: crate::transport::RelayCarrier,
    payload: &[u8],
    plan: MieruManagedUdpFlowPlan<'_>,
) -> Result<FlowStartResult, FlowFailure> {
    let (tag, server, port, resume) = plan.into_parts();
    let mut context = dispatch.flow_start_context();
    start_relay_managed_stream_packet(
        &mut context,
        ManagedStreamPacketStartBridge::relay(
            None,
            tag,
            session,
            ManagedStreamPacketRelay {
                carrier,
                tls_server_name: None,
            },
            (server, port),
            resume,
            payload,
        ),
    )
    .await
}
