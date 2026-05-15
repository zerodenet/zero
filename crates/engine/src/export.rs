use serde::Serialize;
use zero_config::{
    InboundConfig, InboundProtocolConfig, ModeConfig, OutboundConfig, OutboundProtocolConfig,
};
use zero_core::{Address, Network, ProtocolType};

use super::completed_sessions::CompletedSessionRecord;
use super::groups::OutboundGroupStateStore;
use super::plan::{EnginePlan, TargetId, TargetKind};
use super::resolve::resolve_target_chains;
use super::runtime::Engine;
use super::session_registry::ActiveSession;
use super::stats::{EngineStatsSnapshot, SessionOutcome};
use super::view::PlanView;

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
    pub latency_ms: Option<u64>,
    pub last_checked_unix_ms: Option<u64>,
    pub effective_chains: Vec<Vec<String>>,
    pub urltest_members: Vec<UrlTestMemberExport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UrlTestMemberExport {
    pub member_tag: String,
    pub healthy: bool,
    pub latency_ms: Option<u64>,
    pub last_checked_unix_ms: Option<u64>,
    pub last_error: Option<String>,
    pub effective_chains: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActiveSessionExport {
    pub id: u64,
    pub inbound_tag: Option<String>,
    pub outbound_tag: Option<String>,
    pub target: AddressExport,
    pub port: u16,
    pub protocol: String,
    pub auth: Option<SessionAuthExport>,
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
    pub auth: Option<SessionAuthExport>,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SessionAuthExport {
    pub scheme: String,
    pub credential_id: Option<String>,
    pub principal_key: Option<String>,
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
                .map(|group| {
                    let plan = self.plan();
                    let group_id = plan
                        .target_id(group.tag())
                        .expect("engine plan should resolve outbound group");
                    OutboundGroupExport::new(&plan, &self.outbound_group_state, group_id)
                })
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
            OutboundProtocolConfig::Socks5 { server, port, .. } => Self {
                tag: outbound.tag.clone(),
                protocol: "socks5".to_owned(),
                server: Some(server.clone()),
                port: Some(*port),
            },
            OutboundProtocolConfig::Vless { server, port, .. } => Self {
                tag: outbound.tag.clone(),
                protocol: "vless".to_owned(),
                server: Some(server.clone()),
                port: Some(*port),
            },
            OutboundProtocolConfig::Hysteria2 { server, port, .. } => Self {
                tag: outbound.tag.clone(),
                protocol: "hysteria2".to_owned(),
                server: Some(server.clone()),
                port: Some(*port),
            },
            OutboundProtocolConfig::Shadowsocks { server, port, .. } => Self {
                tag: outbound.tag.clone(),
                protocol: "shadowsocks".to_owned(),
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

impl OutboundGroupExport {
    fn new(plan: &EnginePlan, state: &OutboundGroupStateStore, group_id: TargetId) -> Self {
        let view = PlanView::new(plan);
        let group = plan
            .target(group_id)
            .expect("engine plan should resolve outbound group");
        let effective_chains =
            view.render_target_chains(&resolve_target_chains(plan, state, group_id));

        match group.kind() {
            TargetKind::Selector(selector) => Self {
                tag: group.tag().to_owned(),
                kind: "selector".to_owned(),
                outbounds: view.target_tags(selector.members()),
                selected: state
                    .selector_selected_target(group_id)
                    .map(|selected_id| view.target_tag_owned(selected_id))
                    .or_else(|| Some(view.target_tag_owned(selector.initial_member()))),
                latency_ms: None,
                last_checked_unix_ms: None,
                effective_chains,
                urltest_members: Vec::new(),
            },
            TargetKind::Fallback(fallback) => Self {
                tag: group.tag().to_owned(),
                kind: "fallback".to_owned(),
                outbounds: view.target_tags(fallback.members()),
                selected: fallback
                    .members()
                    .first()
                    .map(|member_id| view.target_tag_owned(*member_id)),
                latency_ms: None,
                last_checked_unix_ms: None,
                effective_chains,
                urltest_members: Vec::new(),
            },
            TargetKind::UrlTest(urltest) => {
                let runtime = state.urltest_state(group_id);
                Self {
                    tag: group.tag().to_owned(),
                    kind: "urltest".to_owned(),
                    outbounds: view.target_tags(urltest.members()),
                    selected: runtime
                        .as_ref()
                        .map(|current| view.target_tag_owned(current.selected))
                        .or_else(|| Some(view.target_tag_owned(urltest.initial_member()))),
                    latency_ms: runtime.as_ref().and_then(|current| current.latency_ms),
                    last_checked_unix_ms: runtime
                        .as_ref()
                        .and_then(|current| current.last_checked_unix_ms),
                    effective_chains,
                    urltest_members: urltest
                        .members()
                        .iter()
                        .map(|member_id| {
                            UrlTestMemberExport::new(plan, *member_id, runtime.as_ref())
                        })
                        .collect(),
                }
            }
            TargetKind::Relay(relay) => Self {
                tag: group.tag().to_owned(),
                kind: "relay".to_owned(),
                outbounds: view.target_tags(relay.chain()),
                selected: relay.chain().first().map(|id| view.target_tag_owned(*id)),
                latency_ms: None,
                last_checked_unix_ms: None,
                effective_chains,
                urltest_members: Vec::new(),
            },
            TargetKind::Outbound(_) => {
                unreachable!("outbound group export requires a group target")
            }
        }
    }
}

impl UrlTestMemberExport {
    fn new(
        plan: &EnginePlan,
        member_id: TargetId,
        runtime: Option<&super::groups::UrlTestGroupState>,
    ) -> Self {
        let view = PlanView::new(plan);
        let member_tag = view.target_tag(member_id);
        let member_state = runtime.and_then(|runtime| {
            runtime
                .members
                .iter()
                .find(|member| member.member_id == member_id)
        });

        Self {
            member_tag: member_tag.to_owned(),
            healthy: member_state.map(|member| member.healthy).unwrap_or(false),
            latency_ms: member_state.and_then(|member| member.latency_ms),
            last_checked_unix_ms: member_state.and_then(|member| member.last_checked_unix_ms),
            last_error: member_state.and_then(|member| member.last_error.clone()),
            effective_chains: member_state
                .map(|member| view.render_target_chains(&member.effective_chains))
                .unwrap_or_default(),
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
            auth: session.auth.as_ref().map(SessionAuthExport::from),
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
            auth: session.auth.as_ref().map(SessionAuthExport::from),
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

impl From<&zero_core::SessionAuth> for SessionAuthExport {
    fn from(auth: &zero_core::SessionAuth) -> Self {
        Self {
            scheme: auth.scheme.clone(),
            credential_id: auth.credential_id.clone(),
            principal_key: auth.principal_key.clone(),
        }
    }
}

fn inbound_protocol_name(protocol: &InboundProtocolConfig) -> &'static str {
    match protocol {
        InboundProtocolConfig::Socks5 { .. } => "socks5",
        InboundProtocolConfig::HttpConnect => "http-connect",
        InboundProtocolConfig::Mixed { .. } => "mixed",
        InboundProtocolConfig::Vless { .. } => "vless",
        InboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
        InboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
    }
}

fn protocol_name(protocol: ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::Socks5 => "socks5",
        ProtocolType::HttpConnect => "http-connect",
        ProtocolType::Vless => "vless",
        ProtocolType::Hysteria2 => "hysteria2",
        ProtocolType::Shadowsocks => "shadowsocks",
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
