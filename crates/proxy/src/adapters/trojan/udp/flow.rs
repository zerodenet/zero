use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::trojan::TrojanAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedStreamPacketStart, UdpDispatch,
};
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
    let resume = trojan::udp::udp_flow_resume_from_config(
        password,
        *sni,
        *insecure,
        *client_fingerprint,
        request.relay_chain,
    );
    request
        .dispatch
        .start_tracked_managed_stream_packet(ManagedStreamPacketStart {
            proxy: Some(request.proxy),
            tag,
            session: request.session,
            carrier: request.carrier,
            tls_server_name: None,
            server,
            port: *port,
            resume,
            payload: request.payload,
            relay_chain: request.relay_chain,
        })
        .await
}
