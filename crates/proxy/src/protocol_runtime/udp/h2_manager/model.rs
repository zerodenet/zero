use super::super::ChainTask;
use super::super::H2UdpPeer;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use zero_core::Address;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct H2Key {
    server: String,
    port: u16,
    password: String,
}

impl H2Key {
    pub(super) fn from_peer(peer: &H2UdpPeer<'_>) -> Self {
        Self {
            server: peer.endpoint.server.to_owned(),
            port: peer.endpoint.port,
            password: peer.password.to_owned(),
        }
    }
}

pub(super) struct H2Entry {
    pub(super) send_tx: mpsc::Sender<Vec<u8>>,
}

pub(crate) struct H2SendExisting<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) client_fingerprint: Option<&'a str>,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}
