use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use zero_core::{Address, Session, UdpFlowPacket};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct TrojanKey(trojan::TrojanUdpCacheKey);

impl TrojanKey {
    pub(super) fn from_resume(
        resume: &trojan::TrojanUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Self {
        Self(resume.cache_key(server, port, session_id))
    }

    pub(super) fn relay(session_id: u64) -> Self {
        Self(trojan::TrojanUdpCacheKey::relay(session_id))
    }
}

pub(super) struct TrojanEntry {
    pub(super) send_tx: mpsc::Sender<UdpFlowPacket>,
    pub(super) recv_tx: broadcast::Sender<UdpFlowPacket>,
}

pub(super) struct TrojanUdpPeer<'a> {
    pub(super) endpoint: OutboundEndpoint<'a>,
    pub(super) resume: &'a trojan::TrojanUdpFlowResume,
    pub(super) relay: bool,
}

pub(super) struct TrojanSendExisting<'a> {
    pub(super) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(super) session_id: u64,
    pub(super) proxy: &'a Proxy,
    pub(super) session: &'a Session,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: trojan::TrojanUdpFlowResume,
    pub(super) target: &'a Address,
    pub(super) target_port: u16,
    pub(super) payload: &'a [u8],
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

pub(super) struct TrojanRelayExisting<'a> {
    pub(super) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(super) session_id: u64,
    pub(super) stream: TcpRelayStream,
    pub(super) tls_server_name: Option<&'a str>,
    pub(super) proxy: &'a Proxy,
    pub(super) session: &'a Session,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: trojan::TrojanUdpFlowResume,
    pub(super) target: &'a Address,
    pub(super) target_port: u16,
    pub(super) payload: &'a [u8],
}
