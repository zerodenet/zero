use std::sync::Arc;

use zero_core::Address;
use zero_transport::shadowsocks_transport::ShadowsocksUdpSocketFlow;

use super::bridge::BridgeWaiters;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct SsKey(shadowsocks::ShadowsocksUdpLeafKey);

impl SsKey {
    pub(super) fn new(leaf_key: shadowsocks::ShadowsocksUdpLeafKey) -> Self {
        Self(leaf_key)
    }
}

pub(super) struct SsUpstream {
    pub(super) flow: Arc<ShadowsocksUdpSocketFlow>,
    pub(super) waiters: BridgeWaiters,
}

pub(super) struct SsUdpPeer<'a> {
    pub(super) endpoint: OutboundEndpoint<'a>,
    pub(super) leaf_key: shadowsocks::ShadowsocksUdpLeafKey,
}

pub(super) struct SsSendExisting<'a> {
    pub(super) chain_tasks:
        &'a mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
    pub(super) session_id: u64,
    pub(super) proxy: &'a Proxy,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: shadowsocks::ShadowsocksUdpFlowResume,
    pub(super) target: &'a Address,
    pub(super) target_port: u16,
    pub(super) payload: &'a [u8],
}
