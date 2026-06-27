use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedStreamPacketStart, UdpDispatch,
};
use crate::runtime::Proxy;

pub(super) async fn start(
    adapter: &MieruAdapter,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    start_with_carrier(MieruUdpFlowStart {
        adapter,
        dispatch,
        proxy: Some(proxy),
        session,
        carrier: None,
        leaf,
        payload,
        relay_chain: false,
    })
    .await
}

pub(super) async fn start_relay_final_hop(
    adapter: &MieruAdapter,
    dispatch: &mut UdpDispatch,
    session: &Session,
    carrier: crate::transport::RelayCarrier,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    start_with_carrier(MieruUdpFlowStart {
        adapter,
        dispatch,
        proxy: None,
        session,
        carrier: Some(carrier),
        leaf,
        payload,
        relay_chain: true,
    })
    .await
}

struct MieruUdpFlowStart<'a> {
    adapter: &'a MieruAdapter,
    dispatch: &'a mut UdpDispatch,
    proxy: Option<&'a Proxy>,
    session: &'a Session,
    carrier: Option<crate::transport::RelayCarrier>,
    leaf: &'a ResolvedLeafOutbound<'a>,
    payload: &'a [u8],
    relay_chain: bool,
}

async fn start_with_carrier(
    request: MieruUdpFlowStart<'_>,
) -> Result<FlowStartResult, FlowFailure> {
    let ResolvedLeafOutbound::Mieru {
        tag,
        server,
        port,
        username,
        password,
    } = request.leaf
    else {
        return Err(unreachable_udp_leaf(request.adapter.name(), request.leaf));
    };
    let config = mieru::MieruUdpFlowConfig::new(username, password);
    request
        .dispatch
        .start_tracked_managed_stream_packet(ManagedStreamPacketStart {
            proxy: request.proxy,
            tag,
            session: request.session,
            carrier: request.carrier,
            tls_server_name: None,
            server,
            port: *port,
            resume: config.flow_resume(request.relay_chain),
            payload: request.payload,
            relay_chain: request.relay_chain,
        })
        .await
}
