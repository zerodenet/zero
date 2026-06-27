use zero_core::Address;

use crate::runtime::Proxy;

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
