use crate::runtime::udp_flow::packet_path::ChainTask;
use tokio::task::JoinSet;
use zero_core::Address;

pub(super) struct H2Entry {
    pub(super) sender: hysteria2::Hysteria2UdpFlowSender,
}

pub(super) struct H2SendExisting<'a> {
    pub(super) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(super) session_id: u64,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: hysteria2::Hysteria2UdpFlowResume,
    pub(super) target: &'a Address,
    pub(super) target_port: u16,
    pub(super) payload: &'a [u8],
}
