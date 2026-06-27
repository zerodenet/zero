use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedDatagramStart, UdpDispatch,
};
use crate::runtime::Proxy;

pub(super) async fn start(
    adapter: &ShadowsocksAdapter,
    dispatch: &mut UdpDispatch,
    proxy: &Proxy,
    session: &Session,
    leaf: &ResolvedLeafOutbound<'_>,
    payload: &[u8],
) -> Result<FlowStartResult, FlowFailure> {
    let ResolvedLeafOutbound::Shadowsocks {
        tag,
        server,
        port,
        password,
        cipher,
        ..
    } = leaf
    else {
        return Err(unreachable_udp_leaf(adapter.name(), leaf));
    };
    let resume = shadowsocks::udp_flow_resume_from_config(tag, server, *port, cipher, password)
        .map_err(|error| FlowFailure {
            stage: "udp_shadowsocks_resume",
            error: zero_engine::EngineError::Io(std::io::Error::other(error.to_string())),
            upstream: Some((server.to_string(), *port)),
        })?;
    dispatch
        .start_tracked_managed_datagram(ManagedDatagramStart {
            proxy: Some(proxy),
            tag,
            session,
            server,
            port: *port,
            resume,
            payload,
        })
        .await
}
