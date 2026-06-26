use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinSet;
use zero_core::{Address, Session, UdpFlowPacket};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum TrojanKey {
    Leaf(trojan::TrojanUdpLeafKey),
    Relay { session_id: u64 },
}

impl TrojanKey {
    pub(super) fn from_flow_key(flow_key: trojan::TrojanUdpFlowKey, session_id: u64) -> Self {
        match flow_key {
            trojan::TrojanUdpFlowKey::Leaf(leaf_key) => Self::Leaf(leaf_key),
            trojan::TrojanUdpFlowKey::Relay => Self::Relay { session_id },
        }
    }
}

pub(super) struct TrojanEntry {
    pub(super) send_tx: mpsc::Sender<UdpFlowPacket>,
    pub(super) recv_tx: broadcast::Sender<UdpFlowPacket>,
}

pub(super) struct TrojanUdpPeer<'a> {
    pub(super) endpoint: OutboundEndpoint<'a>,
    pub(super) resume: &'a trojan::TrojanUdpFlowResume,
    pub(super) flow_key: trojan::TrojanUdpFlowKey,
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
