use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::packet_path::{UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use tokio::task::JoinSet;
use zero_core::{Address, Session};

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
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: &'a trojan::TrojanUdpFlowResume,
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
