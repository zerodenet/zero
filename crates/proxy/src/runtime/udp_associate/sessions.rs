use std::collections::HashMap;
use std::net::SocketAddr;

use zero_core::{Address, Session};

use zero_engine::CompletedSessionRecord;
use zero_engine::SessionHandle;
use zero_engine::SessionOutcome;

pub(crate) use crate::runtime::orchestration::UdpPathCategory;

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

/// UDP outbound path category.
///
/// Classifies every `UdpFlowOutbound` variant into one of four transport
/// categories used by the UDP dispatch runtime:
///
/// | Category | Variants | Transport |
/// |----------|----------|-----------|
/// | `Direct` | `Direct` | Raw socket, no upstream manager |
/// | `Relay` | `Socks5` | UDP ASSOCIATE relay through control stream |
/// | `StreamPacket` | `Trojan`, `Mieru` | UDP packets over established stream |
/// | `Datagram` | `Shadowsocks`, `Hysteria2` | Datagram encode/decode over socket or QUIC |
/// Outbound type tracked per UDP flow.
///
/// Variant layout follows the path category model:
///
/// - **Direct path**: raw socket send, no upstream manager.
/// - **Relay path**: `Socks5` UDP ASSOCIATE relay through a control stream.
/// - **Stream packet path**: `Trojan`, `Mieru`: UDP packets sent over an
///   already established encrypted stream.
/// - **Datagram path**: `Shadowsocks`, `Hysteria2`: protocol datagrams
///   encoded and sent over a raw UDP socket or QUIC connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UdpFlowOutbound {
    // Direct path.
    Direct {
        tag: String,
        target_addr: SocketAddr,
    },

    // Relay path.
    Socks5 {
        tag: String,
        server: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },

    // Datagram path.
    #[allow(dead_code)]
    Shadowsocks {
        tag: String,
        server: String,
        port: u16,
        password: String,
        cipher: String,
        packet_path_carrier: Option<UdpPacketPathCarrier>,
    },
    Hysteria2 {
        tag: String,
        server: String,
        port: u16,
        password: String,
        client_fingerprint: Option<String>,
    },

    // Stream packet path.
    Trojan {
        tag: String,
        server: String,
        port: u16,
        password: String,
        sni: Option<String>,
        insecure: bool,
        client_fingerprint: Option<String>,
        relay_chain: bool,
    },
    Mieru {
        tag: String,
        server: String,
        port: u16,
        username: String,
        password: String,
        relay_chain: bool,
    },
}

/// Carrier parameters for a UDP packet path relay chain hop.
///
/// Stores the connection parameters for the packet path provider so that an
/// existing flow can re-dispatch packets through the same carrier without
/// re-resolving the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UdpPacketPathCarrier {
    #[cfg(feature = "socks5")]
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
            Self::Direct { tag, .. }
            | Self::Socks5 { tag, .. }
            | Self::Shadowsocks { tag, .. }
            | Self::Hysteria2 { tag, .. }
            | Self::Trojan { tag, .. }
            | Self::Mieru { tag, .. } => tag,
        }
    }

    /// Return the path category for this outbound.
    pub(crate) fn path_category(&self) -> UdpPathCategory {
        match self {
            Self::Direct { .. } => UdpPathCategory::Direct,
            Self::Socks5 { .. } => UdpPathCategory::Relay,
            Self::Shadowsocks { .. } | Self::Hysteria2 { .. } => UdpPathCategory::Datagram,
            Self::Trojan { .. } | Self::Mieru { .. } => UdpPathCategory::StreamPacket,
        }
    }

    fn upstream_endpoint(&self) -> Option<(String, u16)> {
        match self {
            Self::Direct { .. } => None,
            Self::Socks5 { server, port, .. }
            | Self::Shadowsocks { server, port, .. }
            | Self::Hysteria2 { server, port, .. }
            | Self::Trojan { server, port, .. }
            | Self::Mieru { server, port, .. } => Some((server.clone(), *port)),
        }
    }

    fn success_outcome(&self) -> SessionOutcome {
        match self {
            Self::Direct { .. } => SessionOutcome::DirectRelayed,
            Self::Socks5 { .. }
            | Self::Shadowsocks { .. }
            | Self::Hysteria2 { .. }
            | Self::Trojan { .. }
            | Self::Mieru { .. } => SessionOutcome::ChainedRelayed,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct UdpFlowSnapshot {
    pub(crate) session: Session,
    pub(crate) outbound: UdpFlowOutbound,
    /// Client session isolation key (SIP022 3.2.4).
    pub(crate) client_session_id: Option<u64>,
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
            UdpFlowOutbound::Socks5 { tag, .. }
            | UdpFlowOutbound::Shadowsocks { tag, .. }
            | UdpFlowOutbound::Hysteria2 { tag, .. }
            | UdpFlowOutbound::Trojan { tag, .. }
            | UdpFlowOutbound::Mieru { tag, .. } => {
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
            UdpFlowOutbound::Socks5 { tag, .. }
            | UdpFlowOutbound::Shadowsocks { tag, .. }
            | UdpFlowOutbound::Hysteria2 { tag, .. }
            | UdpFlowOutbound::Trojan { tag, .. }
            | UdpFlowOutbound::Mieru { tag, .. } => {
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
            UdpFlowOutbound::Socks5 { tag, .. }
            | UdpFlowOutbound::Shadowsocks { tag, .. }
            | UdpFlowOutbound::Hysteria2 { tag, .. }
            | UdpFlowOutbound::Trojan { tag, .. }
            | UdpFlowOutbound::Mieru { tag, .. } => tag == outbound_tag,
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
