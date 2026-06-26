use std::sync::Arc;

use zero_core::Address;
use zero_transport::shadowsocks_transport::ShadowsocksUdpSocketFlow;

use super::bridge::BridgeWaiters;
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

pub(crate) struct SsSendExisting<'a> {
    pub(crate) chain_tasks: &'a mut tokio::task::JoinSet<super::super::ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) proxy: &'a Proxy,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: shadowsocks::ShadowsocksUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}
