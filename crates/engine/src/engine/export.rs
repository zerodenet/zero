use serde::Serialize;
use zero_config::{
    InboundConfig, InboundProtocolConfig, ModeConfig, OutboundConfig, OutboundGroupConfig,
    OutboundGroupKind, OutboundProtocolConfig,
};
use zero_core::{Address, Network, ProtocolType};

use super::completed_sessions::CompletedSessionRecord;
use super::runtime::Engine;
use super::session_registry::ActiveSession;
use super::stats::{EngineStatsSnapshot, SessionOutcome};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineConfigExport {
    pub mode: ModeExport,
    pub rule_count: usize,
    pub inbounds: Vec<InboundExport>,
    pub outbounds: Vec<OutboundExport>,
    pub outbound_groups: Vec<OutboundGroupExport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineRuntimeExport {
    pub stats: EngineStatsSnapshot,
    pub udp_upstream_idle_timeout_seconds: u64,
    pub active_sessions: Vec<ActiveSessionExport>,
    pub recent_completed_sessions: Vec<CompletedSessionExport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineStatusExport {
    pub config: EngineConfigExport,
    pub runtime: EngineRuntimeExport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InboundExport {
    pub tag: String,
    pub protocol: String,
    pub listen_address: String,
    pub listen_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutboundExport {
    pub tag: String,
    pub protocol: String,
    pub server: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModeExport {
    pub kind: String,
    pub outbound: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OutboundGroupExport {
    pub tag: String,
    pub kind: String,
    pub outbounds: Vec<String>,
    pub selected: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActiveSessionExport {
    pub id: u64,
    pub inbound_tag: Option<String>,
    pub outbound_tag: Option<String>,
    pub target: AddressExport,
    pub port: u16,
    pub protocol: String,
    pub network: String,
    pub mode: String,
    pub started_at_unix_ms: u64,
    pub last_activity_at_unix_ms: u64,
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub inbound_rx_bytes: u64,
    pub inbound_tx_bytes: u64,
    pub outbound_rx_bytes: u64,
    pub outbound_tx_bytes: u64,
    pub throughput_up_bps: u64,
    pub throughput_down_bps: u64,
    pub process_id: Option<u32>,
    pub process_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompletedSessionExport {
    pub id: u64,
    pub inbound_tag: Option<String>,
    pub outbound_tag: Option<String>,
    pub target: AddressExport,
    pub port: u16,
    pub protocol: String,
    pub network: String,
    pub mode: String,
    pub started_at_unix_ms: u64,
    pub last_activity_at_unix_ms: u64,
    pub finished_at_unix_ms: u64,
    pub duration_ms: u64,
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub inbound_rx_bytes: u64,
    pub inbound_tx_bytes: u64,
    pub outbound_rx_bytes: u64,
    pub outbound_tx_bytes: u64,
    pub process_id: Option<u32>,
    pub process_name: Option<String>,
    pub outcome: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddressExport {
    pub family: String,
    pub value: String,
}

impl Engine {
    pub fn export_config(&self) -> EngineConfigExport {
        EngineConfigExport {
            mode: ModeExport::from(&self.config.mode),
            rule_count: self.config.route.rules.len(),
            inbounds: self
                .config
                .inbounds
                .iter()
                .map(InboundExport::from)
                .collect(),
            outbounds: self
                .config
                .outbounds
                .iter()
                .map(OutboundExport::from)
                .collect(),
            outbound_groups: self
                .config
                .outbound_groups
                .iter()
                .map(OutboundGroupExport::from)
                .collect(),
        }
    }

    pub fn export_runtime(&self) -> EngineRuntimeExport {
        EngineRuntimeExport {
            stats: self.stats_snapshot(),
            udp_upstream_idle_timeout_seconds: self.udp_upstream_idle_timeout().as_secs(),
            active_sessions: self
                .active_sessions()
                .iter()
                .map(ActiveSessionExport::from)
                .collect(),
            recent_completed_sessions: self
                .completed_sessions()
                .iter()
                .map(CompletedSessionExport::from)
                .collect(),
        }
    }

    pub fn export_status(&self) -> EngineStatusExport {
        EngineStatusExport {
            config: self.export_config(),
            runtime: self.export_runtime(),
        }
    }
}

impl From<&InboundConfig> for InboundExport {
    fn from(inbound: &InboundConfig) -> Self {
        Self {
            tag: inbound.tag.clone(),
            protocol: inbound_protocol_name(&inbound.protocol).to_owned(),
            listen_address: inbound.listen.address.clone(),
            listen_port: inbound.listen.port,
        }
    }
}

impl From<&OutboundConfig> for OutboundExport {
    fn from(outbound: &OutboundConfig) -> Self {
        match &outbound.protocol {
            OutboundProtocolConfig::Direct => Self {
                tag: outbound.tag.clone(),
                protocol: "direct".to_owned(),
                server: None,
                port: None,
            },
            OutboundProtocolConfig::Block => Self {
                tag: outbound.tag.clone(),
                protocol: "block".to_owned(),
                server: None,
                port: None,
            },
            OutboundProtocolConfig::Socks5 { server, port } => Self {
                tag: outbound.tag.clone(),
                protocol: "socks5".to_owned(),
                server: Some(server.clone()),
                port: Some(*port),
            },
        }
    }
}

impl From<&ModeConfig> for ModeExport {
    fn from(mode: &ModeConfig) -> Self {
        Self {
            kind: mode.kind().to_owned(),
            outbound: mode.outbound().map(str::to_owned),
        }
    }
}

impl From<&OutboundGroupConfig> for OutboundGroupExport {
    fn from(group: &OutboundGroupConfig) -> Self {
        match &group.group {
            OutboundGroupKind::Selector { outbounds, .. } => Self {
                tag: group.tag.clone(),
                kind: "selector".to_owned(),
                outbounds: outbounds.clone(),
                selected: group.active_outbound().map(str::to_owned),
            },
        }
    }
}

impl From<&ActiveSession> for ActiveSessionExport {
    fn from(session: &ActiveSession) -> Self {
        Self {
            id: session.id,
            inbound_tag: session.inbound_tag.clone(),
            outbound_tag: session.outbound_tag.clone(),
            target: AddressExport::from(&session.target),
            port: session.port,
            protocol: protocol_name(session.protocol).to_owned(),
            network: network_name(session.network).to_owned(),
            mode: session.mode.clone(),
            started_at_unix_ms: session.started_at_unix_ms,
            last_activity_at_unix_ms: session.last_activity_at_unix_ms,
            bytes_up: session.bytes_up,
            bytes_down: session.bytes_down,
            inbound_rx_bytes: session.inbound_rx_bytes,
            inbound_tx_bytes: session.inbound_tx_bytes,
            outbound_rx_bytes: session.outbound_rx_bytes,
            outbound_tx_bytes: session.outbound_tx_bytes,
            throughput_up_bps: session.throughput_up_bps,
            throughput_down_bps: session.throughput_down_bps,
            process_id: session.process_id,
            process_name: session.process_name.clone(),
        }
    }
}

impl From<&CompletedSessionRecord> for CompletedSessionExport {
    fn from(session: &CompletedSessionRecord) -> Self {
        Self {
            id: session.id,
            inbound_tag: session.inbound_tag.clone(),
            outbound_tag: session.outbound_tag.clone(),
            target: AddressExport::from(&session.target),
            port: session.port,
            protocol: protocol_name(session.protocol).to_owned(),
            network: network_name(session.network).to_owned(),
            mode: session.mode.clone(),
            started_at_unix_ms: session.started_at_unix_ms,
            last_activity_at_unix_ms: session.last_activity_at_unix_ms,
            finished_at_unix_ms: session.finished_at_unix_ms,
            duration_ms: session.duration_ms,
            bytes_up: session.bytes_up,
            bytes_down: session.bytes_down,
            inbound_rx_bytes: session.inbound_rx_bytes,
            inbound_tx_bytes: session.inbound_tx_bytes,
            outbound_rx_bytes: session.outbound_rx_bytes,
            outbound_tx_bytes: session.outbound_tx_bytes,
            process_id: session.process_id,
            process_name: session.process_name.clone(),
            outcome: outcome_name(session.outcome).to_owned(),
        }
    }
}

impl From<&Address> for AddressExport {
    fn from(address: &Address) -> Self {
        match address {
            Address::Domain(domain) => Self {
                family: "domain".to_owned(),
                value: domain.clone(),
            },
            Address::Ipv4(addr) => Self {
                family: "ipv4".to_owned(),
                value: format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3]),
            },
            Address::Ipv6(addr) => Self {
                family: "ipv6".to_owned(),
                value: std::net::Ipv6Addr::from(*addr).to_string(),
            },
        }
    }
}

fn inbound_protocol_name(protocol: &InboundProtocolConfig) -> &'static str {
    match protocol {
        InboundProtocolConfig::Socks5 => "socks5",
        InboundProtocolConfig::HttpConnect => "http-connect",
        InboundProtocolConfig::Mixed => "mixed",
    }
}

fn protocol_name(protocol: ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::Socks5 => "socks5",
        ProtocolType::HttpConnect => "http-connect",
        ProtocolType::Unknown => "unknown",
    }
}

fn network_name(network: Network) -> &'static str {
    match network {
        Network::Tcp => "tcp",
        Network::Udp => "udp",
    }
}

fn outcome_name(outcome: SessionOutcome) -> &'static str {
    outcome.kind()
}
