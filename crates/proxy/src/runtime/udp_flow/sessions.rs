use std::collections::HashMap;
use std::net::SocketAddr;

use zero_core::{Address, Session};

use zero_engine::{CompletedSessionRecord, SessionHandle, SessionOutcome};

use super::outbound::UdpFlowOutbound;
use super::snapshot::UdpFlowSnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UdpFlowKey {
    target: Address,
    port: u16,
    /// Per-client-session isolation key.
    ///
    /// When `Some`, flows with the same `(target, port)` but different
    /// `client_session_id` are treated as independent relay sessions (SIP022
    /// 3.2.4). When `None` (legacy AEAD, non-SS protocols), the existing
    /// `(target, port)` keying is preserved.
    client_session_id: Option<u64>,
}

impl UdpFlowKey {
    fn new(target: &Address, port: u16, client_session_id: Option<u64>) -> Self {
        Self {
            target: target.clone(),
            port,
            client_session_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UdpUpstreamResponseKey {
    outbound_tag: String,
    target: Address,
    port: u16,
}

impl UdpUpstreamResponseKey {
    fn new(outbound_tag: &str, target: &Address, port: u16) -> Self {
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
    flows: HashMap<UdpFlowKey, UdpFlow>,
    direct_by_sender: HashMap<SocketAddr, UdpFlowKey>,
    upstream_by_response: HashMap<UdpUpstreamResponseKey, UdpFlowKey>,
}

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
            .filter_map(|(_, flow)| flow.finish_success())
            .collect()
    }

    pub(crate) fn direct_response_session_id(&self, sender: SocketAddr) -> Option<u64> {
        self.direct_by_sender
            .get(&sender)
            .and_then(|key| self.flows.get(key))
            .map(|flow| flow.session.id)
            .or_else(|| self.single_direct_flow_session_id())
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn upstream_response_session_id(
        &self,
        outbound_tag: &str,
        target: &Address,
        port: u16,
    ) -> Option<u64> {
        self.upstream_by_response
            .get(&UdpUpstreamResponseKey::new(outbound_tag, target, port))
            .and_then(|key| self.flows.get(key))
            .map(|flow| flow.session.id)
            .or_else(|| self.single_tagged_upstream_flow_session_id(outbound_tag))
    }

    fn index_flow(&mut self, key: &UdpFlowKey, outbound: &UdpFlowOutbound) {
        let index_keys = outbound.index_keys();
        if let Some(sender) = index_keys.direct_sender {
            self.direct_by_sender.insert(sender, key.clone());
        }

        if let Some(tag) = index_keys.upstream_response_tag {
            self.upstream_by_response.insert(
                UdpUpstreamResponseKey::new(tag, &key.target, key.port),
                key.clone(),
            );
        }
    }

    fn unindex_flow(&mut self, key: &UdpFlowKey, outbound: &UdpFlowOutbound) {
        let index_keys = outbound.index_keys();
        if let Some(sender) = index_keys.direct_sender {
            if self.direct_by_sender.get(&sender) == Some(key) {
                self.direct_by_sender.remove(&sender);
            }
        }

        if let Some(tag) = index_keys.upstream_response_tag {
            let response_key = UdpUpstreamResponseKey::new(tag, &key.target, key.port);
            if self.upstream_by_response.get(&response_key) == Some(key) {
                self.upstream_by_response.remove(&response_key);
            }
        }
    }

    fn single_direct_flow_session_id(&self) -> Option<u64> {
        let mut direct_flows = self
            .flows
            .values()
            .filter(|flow| flow.outbound.index_keys().direct_sender.is_some());
        let flow = direct_flows.next()?;
        direct_flows.next().is_none().then_some(flow.session.id)
    }

    #[cfg(feature = "socks5")]
    fn single_tagged_upstream_flow_session_id(&self, outbound_tag: &str) -> Option<u64> {
        let mut upstream_flows = self
            .flows
            .values()
            .filter(|flow| flow.outbound.index_keys().upstream_response_tag == Some(outbound_tag));
        let flow = upstream_flows.next()?;
        upstream_flows.next().is_none().then_some(flow.session.id)
    }
}

#[derive(Debug)]
struct UdpFlow {
    session: Session,
    handle: SessionHandle,
    outbound: UdpFlowOutbound,
    client_session_id: Option<u64>,
}

impl UdpFlow {
    fn snapshot(&self) -> UdpFlowSnapshot {
        UdpFlowSnapshot {
            session: self.session.clone(),
            outbound: self.outbound.clone(),
            client_session_id: self.client_session_id,
        }
    }

    fn finish(mut self, outcome: SessionOutcome) -> CompletedUdpFlow {
        let upstream = self.outbound.completion().upstream;
        let record = self
            .handle
            .finish(outcome)
            .expect("udp flow should be active before finish");

        CompletedUdpFlow { record, upstream }
    }

    fn finish_success(self) -> Option<CompletedUdpFlow> {
        let outcome = self.outbound.completion().success_outcome;
        Some(self.finish(outcome))
    }
}
