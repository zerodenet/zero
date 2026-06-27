use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, ManagedUdpOutboundKind, ManagedUdpSend, UdpDispatch,
};
use crate::runtime::udp_flow::managed::{ManagedUdpFlowKind, ManagedUdpFlowResume};
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
    let config = shadowsocks::ShadowsocksUdpFlowConfig::new(tag, server, *port, cipher, password);
    let resume = config.flow_resume().map_err(|error| FlowFailure {
        stage: "udp_shadowsocks_resume",
        error: zero_engine::EngineError::Io(std::io::Error::other(error.to_string())),
        upstream: Some((server.to_string(), *port)),
    })?;
    dispatch
        .start_tracked_managed_udp(ManagedUdpSend {
            proxy: Some(proxy),
            tag,
            session,
            carrier: None,
            tls_server_name: None,
            server,
            port: *port,
            resume: ManagedUdpFlowResume::new(resume),
            payload,
            kind: ManagedUdpFlowKind::Datagram,
            outbound: ManagedUdpOutboundKind::Datagram,
        })
        .await
}
