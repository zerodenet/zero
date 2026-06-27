use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, ManagedRelayStart, UdpDispatch};
use crate::runtime::Proxy;

pub(super) async fn start(
    adapter: &Socks5Adapter,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    let ResolvedLeafOutbound::Socks5 {
        tag,
        server,
        port,
        username,
        password,
    } = leaf
    else {
        return Err(unreachable_udp_leaf(adapter.name(), leaf));
    };
    let config = socks5::Socks5UdpPacketPathConfig::new(tag, server, *port, *username, *password);
    dispatch
        .start_tracked_managed_relay(ManagedRelayStart {
            proxy: Some(proxy),
            tag,
            session,
            carrier: None,
            tls_server_name: None,
            server,
            port: *port,
            resume: config.flow_resume(),
            payload,
        })
        .await
        .map_err(|failure| FlowFailure {
            stage: failure.stage,
            error: failure.error,
            upstream: failure.upstream,
        })
}
