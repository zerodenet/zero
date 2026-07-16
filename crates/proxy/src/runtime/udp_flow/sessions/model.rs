use std::collections::HashMap;
use std::net::SocketAddr;

use zero_core::{Address, Session};
use zero_engine::{CompletedSessionRecord, SessionHandle, SessionOutcome};

use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct UdpFlowKey {
    pub(super) target: Address,
    pub(super) port: u16,
    /// Per-client-session isolation key.
    ///
    /// When `Some`, flows with the same `(target, port)` but different
    /// `client_session_id` are treated as independent relay sessions (SIP022
    /// 3.2.4). When `None` (legacy AEAD, non-SS protocols), the existing
    /// `(target, port)` keying is preserved.
    client_session_id: Option<u64>,
}

impl UdpFlowKey {
    pub(super) fn new(target: &Address, port: u16, client_session_id: Option<u64>) -> Self {
        Self {
            target: target.clone(),
            port,
            client_session_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct UdpUpstreamResponseKey {
    outbound_tag: String,
    target: Address,
    port: u16,
}

impl UdpUpstreamResponseKey {
    pub(super) fn new(outbound_tag: &str, target: &Address, port: u16) -> Self {
        Self {
            outbound_tag: outbound_tag.to_owned(),
            target: target.clone(),
            port,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CompletedUdpFlow {
    pub(crate) record: CompletedSessionRecord,
    pub(crate) upstream: Option<(String, u16)>,
}

#[derive(Debug, Default)]
pub(crate) struct UdpSessionFlows {
    pub(super) flows: HashMap<UdpFlowKey, UdpFlow>,
    pub(super) direct_by_sender: HashMap<SocketAddr, UdpFlowKey>,
    pub(super) upstream_by_response: HashMap<UdpUpstreamResponseKey, UdpFlowKey>,
}

#[derive(Debug)]
pub(super) struct UdpFlow {
    pub(super) session: Session,
    pub(super) handle: SessionHandle,
    pub(super) outbound: UdpFlowOutbound,
    pub(super) client_session_id: Option<u64>,
}

impl UdpFlow {
    pub(super) fn snapshot(&self) -> UdpFlowSnapshot {
        UdpFlowSnapshot {
            session: self.session.clone(),
            outbound: self.outbound.clone(),
            client_session_id: self.client_session_id,
        }
    }

    pub(super) fn finish(mut self, outcome: SessionOutcome) -> CompletedUdpFlow {
        let upstream = self.outbound.completion().upstream;
        let record = self
            .handle
            .finish(outcome)
            .expect("udp flow should be active before finish");

        CompletedUdpFlow { record, upstream }
    }

    pub(super) fn finish_success(self) -> CompletedUdpFlow {
        let outcome = self.outbound.completion().success_outcome;
        self.finish(outcome)
    }
}
