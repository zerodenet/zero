use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::trojan::TrojanAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedUdpOutboundKind, ManagedUdpSend, UdpDispatch,
};
use crate::runtime::udp_flow::managed::{ManagedUdpFlowKind, ManagedUdpFlowResume};
use crate::runtime::Proxy;

pub(super) async fn start(
    adapter: &TrojanAdapter,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    start_with_carrier(TrojanUdpFlowStart {
        adapter,
        dispatch,
        proxy,
        session,
        carrier: None,
        leaf,
        payload,
        relay_chain: false,
    })
    .await
}

pub(super) async fn start_relay_final_hop(
    adapter: &TrojanAdapter,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    carrier: crate::transport::RelayCarrier,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    start_with_carrier(TrojanUdpFlowStart {
        adapter,
        dispatch,
        proxy,
        session,
        carrier: Some(carrier),
        leaf,
        payload,
        relay_chain: true,
    })
    .await
}

struct TrojanUdpFlowStart<'a> {
    adapter: &'a TrojanAdapter,
    dispatch: &'a mut UdpDispatch,
    proxy: &'a Proxy,
    session: &'a Session,
    carrier: Option<crate::transport::RelayCarrier>,
    leaf: &'a ResolvedLeafOutbound<'a>,
    payload: &'a [u8],
    relay_chain: bool,
}

async fn start_with_carrier(
    request: TrojanUdpFlowStart<'_>,
) -> Result<FlowStartResult, FlowFailure> {
    let ResolvedLeafOutbound::Trojan {
        tag,
        server,
        port,
        password,
        sni,
        insecure,
        client_fingerprint,
    } = request.leaf
    else {
        return Err(unreachable_udp_leaf(request.adapter.name(), request.leaf));
    };
    let config = trojan::TrojanUdpFlowConfig::new(password, *sni, *insecure, *client_fingerprint);
    request
        .dispatch
        .start_tracked_managed_udp(ManagedUdpSend {
            proxy: Some(request.proxy),
            tag,
            session: request.session,
            carrier: request.carrier,
            tls_server_name: None,
            server,
            port: *port,
            resume: ManagedUdpFlowResume::new(config.flow_resume(request.relay_chain)),
            payload: request.payload,
            kind: if request.relay_chain {
                ManagedUdpFlowKind::RelayStream
            } else {
                ManagedUdpFlowKind::StreamPacket
            },
            outbound: ManagedUdpOutboundKind::StreamPacket,
        })
        .await
}
