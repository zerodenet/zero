use super::super::packet_path_traits::{TrojanUdpPeer, UdpFlowContext, UdpPacketRef};
use super::super::ChainTask;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use zero_core::{Address, Session};

#[derive(Debug, Clone)]
pub(super) struct TrojanPacket {
    pub(super) target: Address,
    pub(super) port: u16,
    pub(super) payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum TrojanKey {
    Leaf {
        server: String,
        port: u16,
        password: String,
    },
    Relay {
        session_id: u64,
    },
}

pub(super) struct TrojanEntry {
    pub(super) send_tx: mpsc::Sender<TrojanPacket>,
    pub(super) recv_tx: broadcast::Sender<TrojanPacket>,
}

pub(crate) struct TrojanSendExisting<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) sni: Option<&'a str>,
    pub(crate) insecure: bool,
    pub(crate) client_fingerprint: Option<&'a str>,
    pub(crate) relay_chain: bool,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}

pub(super) struct TrojanRelaySend<'a> {
    pub(super) ctx: UdpFlowContext<'a>,
    pub(super) stream: TcpRelayStream,
    pub(super) tls_server_name: Option<&'a str>,
    pub(super) proxy: &'a Proxy,
    pub(super) session: &'a Session,
    pub(super) peer: TrojanUdpPeer<'a>,
    pub(super) packet: UdpPacketRef<'a>,
}

pub(crate) struct TrojanRelayExisting<'a> {
    pub(crate) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(crate) session_id: u64,
    pub(crate) stream: TcpRelayStream,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) password: &'a str,
    pub(crate) sni: Option<&'a str>,
    pub(crate) insecure: bool,
    pub(crate) client_fingerprint: Option<&'a str>,
    pub(crate) target: &'a Address,
    pub(crate) target_port: u16,
    pub(crate) payload: &'a [u8],
}
