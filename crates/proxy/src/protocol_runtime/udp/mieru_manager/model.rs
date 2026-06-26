use super::bridge;
use super::stream::MieruFlowSender;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use tokio::task::JoinSet;
use zero_core::{Address, Session};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum MieruKey {
    Leaf(mieru::MieruUdpLeafKey),
    Relay { session_id: u64 },
}

impl MieruKey {
    pub(super) fn from_flow_key(flow_key: mieru::MieruUdpFlowKey, session_id: u64) -> Self {
        match flow_key {
            mieru::MieruUdpFlowKey::Leaf(leaf_key) => Self::Leaf(leaf_key),
            mieru::MieruUdpFlowKey::Relay => Self::Relay { session_id },
        }
    }
}

pub(super) struct MieruEntry {
    pub(super) sender: MieruFlowSender,
    pub(super) recv_tx: bridge::ResponseSender,
}

pub(crate) struct MieruSendExisting<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: mieru::MieruUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct MieruRelayExisting<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) stream: TcpRelayStream,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: mieru::MieruUdpFlowResume,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}
