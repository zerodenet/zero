use super::bridge;
use super::stream::MieruFlowSender;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use tokio::task::JoinSet;
use zero_core::{Address, Session};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct MieruKey(mieru::MieruUdpCacheKey);

impl MieruKey {
    pub(super) fn from_resume(
        resume: &mieru::MieruUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Self {
        Self(resume.cache_key(server, port, session_id))
    }

    pub(super) fn relay(session_id: u64) -> Self {
        Self(mieru::MieruUdpCacheKey::relay(session_id))
    }
}

pub(super) struct MieruEntry {
    pub(super) sender: MieruFlowSender,
    pub(super) recv_tx: bridge::ResponseSender,
}

pub(super) struct MieruUdpPeer<'a> {
    pub(super) endpoint: OutboundEndpoint<'a>,
    pub(super) resume: &'a mieru::MieruUdpFlowResume,
    pub(super) relay: bool,
}

pub(super) struct MieruSendExisting<'a> {
    pub(super) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(super) session_id: u64,
    pub(super) proxy: &'a Proxy,
    pub(super) session: &'a Session,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: mieru::MieruUdpFlowResume,
    pub(super) target: &'a Address,
    pub(super) target_port: u16,
    pub(super) payload: &'a [u8],
}

pub(super) struct MieruRelayExisting<'a> {
    pub(super) chain_tasks: &'a mut JoinSet<ChainTask>,
    pub(super) session_id: u64,
    pub(super) stream: TcpRelayStream,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: mieru::MieruUdpFlowResume,
    pub(super) target: &'a Address,
    pub(super) target_port: u16,
    pub(super) payload: &'a [u8],
}
