use tokio::time::Instant as TokioInstant;
use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::transport::StreamTraffic;

use super::sessions::{UdpFlowOutbound, UdpSessionFlows};
use crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation;

pub(super) struct UdpCandidateContext<'a> {
    pub(super) inbound_tag: &'a str,
    pub(super) relay: &'a TokioDatagramSocket,
    pub(super) session: &'a Session,
    pub(super) payload: &'a [u8],
    pub(super) upstream_association: &'a mut Option<ActiveUpstreamSocks5UdpAssociation>,
    pub(super) upstream_idle_deadline: &'a mut Option<TokioInstant>,
}

pub(super) struct UdpRequestContext<'a> {
    pub(super) inbound_tag: &'a str,
    pub(super) relay: &'a TokioDatagramSocket,
    pub(super) udp_flows: &'a mut UdpSessionFlows,
    pub(super) pending_control_traffic: &'a mut StreamTraffic,
    pub(super) upstream_association: &'a mut Option<ActiveUpstreamSocks5UdpAssociation>,
    pub(super) upstream_idle_deadline: &'a mut Option<TokioInstant>,
}

pub(super) struct ExistingUdpFlowContext<'a> {
    pub(super) inbound_tag: &'a str,
    pub(super) relay: &'a TokioDatagramSocket,
    pub(super) udp_flows: &'a mut UdpSessionFlows,
    pub(super) upstream_association: &'a mut Option<ActiveUpstreamSocks5UdpAssociation>,
    pub(super) upstream_idle_deadline: &'a mut Option<TokioInstant>,
}

pub(super) struct Socks5UdpPacketContext<'a> {
    pub(super) inbound_tag: &'a str,
    pub(super) tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) auth: Option<(&'a str, &'a str)>,
    pub(super) session: &'a Session,
    pub(super) payload: &'a [u8],
    pub(super) upstream_association: &'a mut Option<ActiveUpstreamSocks5UdpAssociation>,
    pub(super) upstream_idle_deadline: &'a mut Option<TokioInstant>,
}

pub(super) enum UdpCandidateStart {
    Flow {
        outbound: UdpFlowOutbound,
        outbound_tx_bytes: u64,
    },
    Blocked {
        tag: String,
    },
}

pub(super) struct UdpCandidateFailure {
    pub(super) stage: &'static str,
    pub(super) error: EngineError,
    pub(super) upstream: Option<(String, u16)>,
}
