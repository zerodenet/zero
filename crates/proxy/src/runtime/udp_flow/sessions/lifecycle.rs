use zero_core::{Address, Session};
use zero_engine::{SessionHandle, SessionOutcome};

use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

use super::model::{CompletedUdpFlow, UdpFlow, UdpFlowKey, UdpSessionFlows};

impl UdpSessionFlows {
    pub(crate) fn snapshot(
        &self,
        target: &Address,
        port: u16,
        client_session_id: Option<u64>,
    ) -> Option<UdpFlowSnapshot> {
        self.flows
            .get(&UdpFlowKey::new(target, port, client_session_id))
            .map(UdpFlow::snapshot)
    }

    /// Look up a session ID by target+port only, regardless of outbound type.
    ///
    /// Used for chain-outbound response metering where the outbound tag
    /// may not be known at the call site.
    #[cfg(feature = "socks5")]
    pub(crate) fn session_id_by_target(
        &self,
        target: &Address,
        port: u16,
        client_session_id: Option<u64>,
    ) -> Option<u64> {
        self.flows
            .get(&UdpFlowKey::new(target, port, client_session_id))
            .map(|flow| flow.session.id)
    }

    pub(crate) fn insert(
        &mut self,
        session: Session,
        handle: SessionHandle,
        outbound: UdpFlowOutbound,
        client_session_id: Option<u64>,
    ) {
        let key = UdpFlowKey::new(&session.target, session.port, client_session_id);
        self.index_flow(&key, &outbound);
        self.flows.insert(
            key,
            UdpFlow {
                session,
                handle,
                outbound,
                client_session_id,
            },
        );
    }

    pub(crate) fn finish(
        &mut self,
        target: &Address,
        port: u16,
        client_session_id: Option<u64>,
        outcome: SessionOutcome,
    ) -> Option<CompletedUdpFlow> {
        let key = UdpFlowKey::new(target, port, client_session_id);
        let flow = self.flows.remove(&key)?;
        self.unindex_flow(&key, &flow.outbound);
        Some(flow.finish(outcome))
    }

    pub(crate) fn finish_all(&mut self) -> Vec<CompletedUdpFlow> {
        self.direct_by_sender.clear();
        self.upstream_by_response.clear();

        self.flows
            .drain()
            .map(|(_, flow)| flow.finish_success())
            .collect()
    }
}
