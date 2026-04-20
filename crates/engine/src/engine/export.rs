use serde::Serialize;
use zero_config::{InboundConfig, InboundProtocolConfig, OutboundConfig, OutboundProtocolConfig};
use zero_core::{Address, ProtocolType};

use super::runtime::Engine;
use super::session_registry::ActiveSession;
use super::stats::EngineStatsSnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineConfigExport {
    pub rule_count: usize,
    pub inbounds: Vec<InboundExport>,
    pub outbounds: Vec<OutboundExport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineRuntimeExport {
    pub stats: EngineStatsSnapshot,
    pub active_sessions: Vec<ActiveSessionExport>,
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
pub struct ActiveSessionExport {
    pub id: u64,
    pub inbound_tag: Option<String>,
    pub outbound_tag: Option<String>,
    pub target: AddressExport,
    pub port: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddressExport {
    pub family: String,
    pub value: String,
}

impl Engine {
    pub fn export_config(&self) -> EngineConfigExport {
        EngineConfigExport {
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
        }
    }

    pub fn export_runtime(&self) -> EngineRuntimeExport {
        EngineRuntimeExport {
            stats: self.stats_snapshot(),
            active_sessions: self
                .active_sessions()
                .iter()
                .map(ActiveSessionExport::from)
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

impl From<&ActiveSession> for ActiveSessionExport {
    fn from(session: &ActiveSession) -> Self {
        Self {
            id: session.id,
            inbound_tag: session.inbound_tag.clone(),
            outbound_tag: session.outbound_tag.clone(),
            target: AddressExport::from(&session.target),
            port: session.port,
            protocol: protocol_name(session.protocol).to_owned(),
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
