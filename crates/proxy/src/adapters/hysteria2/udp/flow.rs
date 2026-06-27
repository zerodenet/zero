use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedUdpOutboundKind, ManagedUdpSend, UdpDispatch,
};
use crate::runtime::udp_flow::managed::{ManagedUdpFlowKind, ManagedUdpFlowResume};

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
    let config = hysteria2::Hysteria2UdpPacketPathConfig::new(
        tag,
        server,
        *port,
        password,
        *client_fingerprint,
    );
    dispatch
        .start_tracked_managed_udp(ManagedUdpSend {
            proxy: None,
            tag,
            session,
            carrier: None,
            tls_server_name: None,
            server,
            port: *port,
            resume: ManagedUdpFlowResume::new(config.flow_resume()),
            payload,
            kind: ManagedUdpFlowKind::Datagram,
            outbound: ManagedUdpOutboundKind::Datagram,
        })
        .await
}
