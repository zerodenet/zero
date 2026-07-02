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
    start_with_carrier(adapter, dispatch, proxy, session, None, leaf, payload).await
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
    start_with_carrier(
        adapter,
        dispatch,
        proxy,
        session,
        Some(carrier),
        leaf,
        payload,
    )
    .await
}

async fn start_with_carrier(
    adapter: &TrojanAdapter,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    carrier: Option<crate::transport::RelayCarrier>,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    let relay_chain = carrier.is_some();
    let ResolvedLeafOutbound::Trojan {
        tag,
        server,
        port,
        password,
        sni,
        insecure,
        client_fingerprint,
    } = leaf
    else {
        return Err(unreachable_udp_leaf(adapter.name(), leaf));
    };
    let resume = trojan::udp::udp_flow_resume_from_config(
        password,
        *sni,
        *insecure,
        *client_fingerprint,
        relay_chain,
    );
    dispatch
        .start_tracked_managed_stream_packet(ManagedStreamPacketStart {
            proxy: Some(proxy),
            tag,
            session,
            carrier,
            tls_server_name: None,
            server,
            port: *port,
            resume,
            payload,
            relay_chain,
        })
        .await
}
