use crate::runtime::Proxy;
use zero_core::Session;

use super::ProtocolUdpFlowResume;

pub(crate) struct ManagedDatagramFlow<'a> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedStreamPacketFlow<'a> {
    pub(crate) chain_tasks:
        &'a mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
    pub(crate) proxy: &'a Proxy,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedRelayStreamFlow<'a> {
    pub(crate) chain_tasks:
        &'a mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>,
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) session: &'a Session,
    pub(crate) carrier: crate::transport::RelayCarrier,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

pub(crate) struct ManagedUdpFlowRequest<'a> {
    pub(crate) chain_tasks:
        Option<&'a mut tokio::task::JoinSet<crate::runtime::udp_flow::packet_path::ChainTask>>,
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) kind: ManagedUdpFlowKind,
    pub(crate) outbound_tag: Option<&'a str>,
    pub(crate) session: &'a Session,
    pub(crate) carrier: Option<crate::transport::RelayCarrier>,
    pub(crate) tls_server_name: Option<&'a str>,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: ProtocolUdpFlowResume,
    pub(crate) payload: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagedUdpFlowKind {
    Datagram,
    StreamPacket,
    RelayStream,
}
