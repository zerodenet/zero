use tokio::task::JoinSet;
use zero_core::{Address, Session};

use crate::protocol_runtime::udp::ProtocolUdpFlowResume;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(in crate::protocol_runtime::udp) struct ManagedExistingSend<'a> {
    pub(in crate::protocol_runtime::udp) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(in crate::protocol_runtime::udp) session_id: u64,
    pub(in crate::protocol_runtime::udp) proxy: Option<&'a Proxy>,
    pub(in crate::protocol_runtime::udp) session: &'a Session,
    pub(in crate::protocol_runtime::udp) server: &'a str,
    pub(in crate::protocol_runtime::udp) port: u16,
    pub(in crate::protocol_runtime::udp) resume: ProtocolUdpFlowResume,
    pub(in crate::protocol_runtime::udp) target: &'a Address,
    pub(in crate::protocol_runtime::udp) target_port: u16,
    pub(in crate::protocol_runtime::udp) payload: &'a [u8],
}

pub(in crate::protocol_runtime::udp) struct ManagedRelaySend<'a> {
    pub(in crate::protocol_runtime::udp) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(in crate::protocol_runtime::udp) session_id: u64,
    pub(in crate::protocol_runtime::udp) stream: TcpRelayStream,
    pub(in crate::protocol_runtime::udp) tls_server_name: Option<&'a str>,
    pub(in crate::protocol_runtime::udp) proxy: Option<&'a Proxy>,
    pub(in crate::protocol_runtime::udp) session: &'a Session,
    pub(in crate::protocol_runtime::udp) server: &'a str,
    pub(in crate::protocol_runtime::udp) port: u16,
    pub(in crate::protocol_runtime::udp) resume: ProtocolUdpFlowResume,
    pub(in crate::protocol_runtime::udp) target: &'a Address,
    pub(in crate::protocol_runtime::udp) target_port: u16,
    pub(in crate::protocol_runtime::udp) payload: &'a [u8],
}
