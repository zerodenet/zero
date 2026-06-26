use super::super::ChainTask;
use super::super::H2UdpPeer;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use zero_core::Address;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct H2Key(hysteria2::Hysteria2UdpLeafKey);

impl H2Key {
    pub(super) fn from_peer(peer: &H2UdpPeer<'_>) -> Self {
        let peer_config = peer.resume.peer_config();
        Self(peer_config.leaf_cache_key(peer.endpoint.server, peer.endpoint.port))
    }
}

pub(super) struct H2Entry {
    pub(super) send_tx: mpsc::Sender<hysteria2::Hysteria2UdpFlowPacket>,
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
