use crate::runtime::udp_flow::packet_path::ChainTask;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use zero_core::{Address, UdpFlowPacket};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum H2Key {
    Leaf(hysteria2::Hysteria2UdpLeafKey),
}

impl H2Key {
    pub(super) fn from_flow_key(flow_key: hysteria2::Hysteria2UdpFlowKey) -> Self {
        match flow_key {
            hysteria2::Hysteria2UdpFlowKey::Leaf(leaf_key) => Self::Leaf(leaf_key),
        }
    }
}

pub(super) struct H2Entry {
    pub(super) send_tx: mpsc::Sender<UdpFlowPacket>,
}

pub(crate) struct H2SendExisting<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: hysteria2::Hysteria2UdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}
