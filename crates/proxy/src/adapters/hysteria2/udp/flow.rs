use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedDatagramStart, UdpDispatch,
};

pub(super) async fn start(
    adapter: &Hysteria2Adapter,
    dispatch: &mut UdpDispatch,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    let ResolvedLeafOutbound::Hysteria2 {
        tag,
        server,
        port,
        password,
        client_fingerprint,
        ..
    } = leaf
    else {
        return Err(unreachable_udp_leaf(adapter.name(), leaf));
    };
    let resume =
        hysteria2::udp_flow_resume_from_config(tag, server, *port, password, *client_fingerprint);
    dispatch
        .start_tracked_managed_datagram(ManagedDatagramStart {
            proxy: None,
            tag,
            session,
            server,
            port: *port,
            resume,
            payload,
        })
        .await
}
