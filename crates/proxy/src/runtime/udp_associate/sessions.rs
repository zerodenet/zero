use std::collections::HashMap;
use std::net::SocketAddr;

use zero_core::{Address, Session};

use zero_engine::CompletedSessionRecord;
use zero_engine::SessionHandle;
use zero_engine::SessionOutcome;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UdpFlowKey {
    target: Address,
    port: u16,
}

impl UdpFlowKey {
    fn new(target: &Address, port: u16) -> Self {
        Self {
            target: target.clone(),
            port,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UdpFlowOutbound {
    Direct {
        tag: String,
        target_addr: SocketAddr,
    },
    Socks5 {
        tag: String,
        server: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    Shadowsocks {
        tag: String,
        server: String,
        port: u16,
        password: String,
        cipher: String,
    },
}

impl UdpFlowOutbound {
    pub(crate) fn tag(&self) -> &str {
        match self {
            Self::Direct { tag, .. } | Self::Socks5 { tag, .. } | Self::Shadowsocks { tag, .. } => {
                tag
            }
        }
    }

    fn upstream_endpoint(&self) -> Option<(String, u16)> {
        match self {
            Self::Direct { .. } => None,
            Self::Socks5 { server, port, .. } | Self::Shadowsocks { server, port, .. } => {
                Some((server.clone(), *port))
            }
        }
    }

    fn success_outcome(&self) -> SessionOutcome {
        match self {
            Self::Direct { .. } => SessionOutcome::DirectRelayed,
            Self::Socks5 { .. } | Self::Shadowsocks { .. } => SessionOutcome::ChainedRelayed,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UdpFlowSnapshot {
    pub(crate) session: Session,
    pub(crate) outbound: UdpFlowOutbound,
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
    pub(crate) fn snapshot(&self, target: &Address, port: u16) -> Option<UdpFlowSnapshot> {
        self.flows
            .get(&UdpFlowKey::new(target, port))
            .map(UdpFlow::snapshot)
    }

    pub(crate) fn insert(
        &mut self,
        session: Session,
        handle: SessionHandle,
        outbound: UdpFlowOutbound,
    ) {
        let key = UdpFlowKey::new(&session.target, session.port);
        self.index_flow(&key, &outbound);
        self.flows.insert(
            key,
            UdpFlow {
                session,
                handle,
                outbound,
            },
        );
    }

    pub(crate) fn finish(
        &mut self,
        target: &Address,
        port: u16,
        outcome: SessionOutcome,
    ) -> Option<CompletedUdpFlow> {
        let key = UdpFlowKey::new(target, port);
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
            .or_else(|| self.single_socks5_flow_session_id(outbound_tag))
    }

    fn index_flow(&mut self, key: &UdpFlowKey, outbound: &UdpFlowOutbound) {
        match outbound {
            UdpFlowOutbound::Direct { target_addr, .. } => {
                self.direct_by_sender.insert(*target_addr, key.clone());
            }
            UdpFlowOutbound::Socks5 { tag, .. } | UdpFlowOutbound::Shadowsocks { tag, .. } => {
                self.upstream_by_response.insert(
                    UdpUpstreamResponseKey::new(tag, &key.target, key.port),
                    key.clone(),
                );
            }
        }
    }

    fn unindex_flow(&mut self, key: &UdpFlowKey, outbound: &UdpFlowOutbound) {
        match outbound {
            UdpFlowOutbound::Direct { target_addr, .. } => {
                if self.direct_by_sender.get(target_addr) == Some(key) {
                    self.direct_by_sender.remove(target_addr);
                }
            }
            UdpFlowOutbound::Socks5 { tag, .. } | UdpFlowOutbound::Shadowsocks { tag, .. } => {
                let response_key = UdpUpstreamResponseKey::new(tag, &key.target, key.port);
                if self.upstream_by_response.get(&response_key) == Some(key) {
                    self.upstream_by_response.remove(&response_key);
                }
            }
        }
    }

    fn single_direct_flow_session_id(&self) -> Option<u64> {
        let mut direct_flows = self
            .flows
            .values()
            .filter(|flow| matches!(flow.outbound, UdpFlowOutbound::Direct { .. }));
        let flow = direct_flows.next()?;
        direct_flows.next().is_none().then_some(flow.session.id)
    }

    fn single_socks5_flow_session_id(&self, outbound_tag: &str) -> Option<u64> {
        let mut upstream_flows = self.flows.values().filter(|flow| match &flow.outbound {
            UdpFlowOutbound::Socks5 { tag, .. } | UdpFlowOutbound::Shadowsocks { tag, .. } => {
                tag == outbound_tag
            }
            UdpFlowOutbound::Direct { .. } => false,
        });
        let flow = upstream_flows.next()?;
        upstream_flows.next().is_none().then_some(flow.session.id)
    }
}

#[derive(Debug)]
struct UdpFlow {
    session: Session,
    handle: SessionHandle,
    outbound: UdpFlowOutbound,
}

impl UdpFlow {
    fn snapshot(&self) -> UdpFlowSnapshot {
        UdpFlowSnapshot {
            session: self.session.clone(),
            outbound: self.outbound.clone(),
        }
    }

    fn finish(mut self, outcome: SessionOutcome) -> CompletedUdpFlow {
        let upstream = self.outbound.upstream_endpoint();
        let record = self
            .handle
            .finish(outcome)
            .expect("udp flow should be active before finish");

        CompletedUdpFlow { record, upstream }
    }

    fn finish_success(self) -> Option<CompletedUdpFlow> {
        let outcome = self.outbound.success_outcome();
        Some(self.finish(outcome))
    }
}
