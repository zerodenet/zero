//! Engine export --converts internal types to `zero-api` snapshot types.
//!
//! Free conversion functions are used instead of `From` impls because the
//! snapshot types live in `zero-api` (foreign crate). This satisfies the
//! orphan rule while keeping all conversion logic in one file.

use zero_api::{
    AddressSnapshot, AuthSnapshot, CompletedFlowSnapshot, ConfigSnapshot, FlowSnapshot,
    ListenerSnapshot, ModeSnapshot, OutboundTargetSnapshot, PolicyMemberSnapshot, PolicySnapshot,
    RuntimeSnapshot, StatusSnapshot,
};
use zero_config::{
    InboundConfig, InboundProtocolConfig, ModeConfig, OutboundConfig, OutboundProtocolConfig,
};

use super::completed_sessions::CompletedSessionRecord;
use super::groups::OutboundGroupStateStore;
use super::plan::{EnginePlan, TargetId};
use super::resolve::resolve_target_chains;
use super::runtime::Engine;
use super::session_registry::ActiveSession;
use super::stats::SessionOutcome;
use super::view::PlanView;
use zero_core::{Address, Network, ProtocolType};

// ── Engine export methods ────────────────────────────────────────────

impl Engine {
    pub fn export_config(&self) -> ConfigSnapshot {
        let config = self.config();
        ConfigSnapshot {
            mode: mode_to_snapshot(&config.mode),
            rule_count: config.route.rules.len(),
            listeners: config.inbounds.iter().map(inbound_to_listener).collect(),
            outbounds: config.outbounds.iter().map(outbound_to_snapshot).collect(),
            outbound_groups: config
                .outbound_groups
                .iter()
                .map(|group| {
                    let plan = self.plan();
                    let group_id = plan
                        .target_id(group.tag())
                        .expect("engine plan should resolve outbound group");
                    build_policy_snapshot(&plan, &self.outbound_group_state, group_id)
                })
                .collect(),
        }
    }

    pub fn export_runtime(&self) -> RuntimeSnapshot {
        let config = self.config();
        RuntimeSnapshot {
            stats: self.stats_snapshot(),
            udp_upstream_idle_timeout_seconds: self.udp_upstream_idle_timeout().as_secs(),
            log_level: config.runtime.log.level.clone(),
            log_files: config
                .runtime
                .log
                .files
                .iter()
                .map(|f| f.path.clone())
                .collect(),
            pid: self.pid,
            config_path: self.config_path().map(|p| p.display().to_string()),
            active_sessions: self.active_sessions().iter().map(session_to_flow).collect(),
            recent_completed_sessions: self
                .completed_sessions()
                .iter()
                .map(completed_to_flow)
                .collect(),
            started_at_unix_ms: self.started_at_unix_ms(),
        }
    }

    pub fn export_status(&self) -> StatusSnapshot {
        StatusSnapshot {
            config: self.export_config(),
            runtime: self.export_runtime(),
        }
    }
}

// ── Session conversions ─────────────────────────────────────────────

pub(crate) fn session_to_flow(session: &ActiveSession) -> FlowSnapshot {
    FlowSnapshot {
        id: session.id,
        inbound_tag: session.inbound_tag.clone(),
        outbound_tag: session.outbound_tag.clone(),
        target: address_to_snapshot(&session.target),
        port: session.port,
        protocol: protocol_name(session.protocol).to_owned(),
        auth: session.auth.as_ref().map(auth_to_snapshot),
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

pub(crate) fn completed_to_flow(session: &CompletedSessionRecord) -> CompletedFlowSnapshot {
    CompletedFlowSnapshot {
        id: session.id,
        inbound_tag: session.inbound_tag.clone(),
        outbound_tag: session.outbound_tag.clone(),
        target: address_to_snapshot(&session.target),
        port: session.port,
        protocol: protocol_name(session.protocol).to_owned(),
        auth: session.auth.as_ref().map(auth_to_snapshot),
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
        close_reason: session.close_reason.clone(),
    }
}

pub(crate) fn address_to_snapshot(address: &Address) -> AddressSnapshot {
    match address {
        Address::Domain(domain) => AddressSnapshot {
            family: "domain".to_owned(),
            value: domain.clone(),
        },
        Address::Ipv4(addr) => AddressSnapshot {
            family: "ipv4".to_owned(),
            value: format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3]),
        },
        Address::Ipv6(addr) => AddressSnapshot {
            family: "ipv6".to_owned(),
            value: std::net::Ipv6Addr::from(*addr).to_string(),
        },
    }
}

pub(crate) fn auth_to_snapshot(auth: &zero_core::SessionAuth) -> AuthSnapshot {
    AuthSnapshot {
        scheme: auth.scheme.clone(),
        credential_id: auth.credential_id.clone(),
        principal_key: auth.principal_key.clone(),
    }
}

// ── Config conversions ──────────────────────────────────────────────

fn inbound_to_listener(inbound: &InboundConfig) -> ListenerSnapshot {
    ListenerSnapshot {
        tag: inbound.tag.clone(),
        protocol: inbound_protocol_name(&inbound.protocol).to_owned(),
        listen_address: inbound.listen.address.clone(),
        listen_port: inbound.listen.port,
    }
}

fn outbound_to_snapshot(outbound: &OutboundConfig) -> OutboundTargetSnapshot {
    match &outbound.protocol {
        OutboundProtocolConfig::Direct => OutboundTargetSnapshot {
            tag: outbound.tag.clone(),
            protocol: "direct".to_owned(),
            server: None,
            port: None,
        },
        OutboundProtocolConfig::Block => OutboundTargetSnapshot {
            tag: outbound.tag.clone(),
            protocol: "block".to_owned(),
            server: None,
            port: None,
        },
        OutboundProtocolConfig::Socks5 { server, port, .. } => OutboundTargetSnapshot {
            tag: outbound.tag.clone(),
            protocol: "socks5".to_owned(),
            server: Some(server.clone()),
            port: Some(*port),
        },
        OutboundProtocolConfig::Vless { server, port, .. } => OutboundTargetSnapshot {
            tag: outbound.tag.clone(),
            protocol: "vless".to_owned(),
            server: Some(server.clone()),
            port: Some(*port),
        },
        OutboundProtocolConfig::Hysteria2 { server, port, .. } => OutboundTargetSnapshot {
            tag: outbound.tag.clone(),
            protocol: "hysteria2".to_owned(),
            server: Some(server.clone()),
            port: Some(*port),
        },
        OutboundProtocolConfig::Shadowsocks { server, port, .. } => OutboundTargetSnapshot {
            tag: outbound.tag.clone(),
            protocol: "shadowsocks".to_owned(),
            server: Some(server.clone()),
            port: Some(*port),
        },
        OutboundProtocolConfig::Trojan { server, port, .. } => OutboundTargetSnapshot {
            tag: outbound.tag.clone(),
            protocol: "trojan".to_owned(),
            server: Some(server.clone()),
            port: Some(*port),
        },
        OutboundProtocolConfig::Vmess { server, port, .. } => OutboundTargetSnapshot {
            tag: outbound.tag.clone(),
            protocol: "vmess".to_owned(),
            server: Some(server.clone()),
            port: Some(*port),
        },
        OutboundProtocolConfig::Mieru { server, port, .. } => OutboundTargetSnapshot {
            tag: outbound.tag.clone(),
            protocol: "mieru".to_owned(),
            server: Some(server.clone()),
            port: Some(*port),
        },
    }
}

fn mode_to_snapshot(mode: &ModeConfig) -> ModeSnapshot {
    ModeSnapshot {
        kind: mode.kind().to_owned(),
        outbound: mode.outbound().map(str::to_owned),
    }
}

// ── Policy conversion ───────────────────────────────────────────────

fn build_policy_snapshot(
    plan: &EnginePlan,
    state: &OutboundGroupStateStore,
    group_id: TargetId,
) -> PolicySnapshot {
    let view = PlanView::new(plan);
    let group = plan
        .target(group_id)
        .expect("engine plan should resolve outbound group");
    let effective_chains = view.render_target_chains(&resolve_target_chains(plan, state, group_id));

    if let Some(selector) = group.as_selector() {
        PolicySnapshot {
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
            url_test_members: Vec::new(),
        }
    } else if let Some(fallback) = group.as_fallback() {
        PolicySnapshot {
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
            url_test_members: Vec::new(),
        }
    } else if let Some(urltest) = group.as_urltest() {
        let runtime = state.urltest_state(group_id);
        PolicySnapshot {
            tag: group.tag().to_owned(),
            kind: "url_test".to_owned(),
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
            url_test_members: urltest
                .members()
                .iter()
                .map(|member_id| build_policy_member_snapshot(plan, *member_id, runtime.as_ref()))
                .collect(),
        }
    } else if let Some(relay) = group.as_relay() {
        PolicySnapshot {
            tag: group.tag().to_owned(),
            kind: "relay".to_owned(),
            outbounds: view.target_tags(relay.chain()),
            selected: relay.chain().first().map(|id| view.target_tag_owned(*id)),
            latency_ms: None,
            last_checked_unix_ms: None,
            effective_chains,
            url_test_members: Vec::new(),
        }
    } else if let Some(lb) = group.as_loadbalance() {
        PolicySnapshot {
            tag: group.tag().to_owned(),
            kind: "load_balance".to_owned(),
            outbounds: view.target_tags(lb.members()),
            selected: lb
                .members()
                .first()
                .map(|member_id| view.target_tag_owned(*member_id)),
            latency_ms: None,
            last_checked_unix_ms: None,
            effective_chains,
            url_test_members: Vec::new(),
        }
    } else {
        unreachable!("outbound group export requires a group target")
    }
}

fn build_policy_member_snapshot(
    plan: &EnginePlan,
    member_id: TargetId,
    runtime: Option<&super::groups::UrlTestGroupState>,
) -> PolicyMemberSnapshot {
    let view = PlanView::new(plan);
    let member_tag = view.target_tag(member_id);
    let member_state = runtime.and_then(|runtime| {
        runtime
            .members
            .iter()
            .find(|member| member.member_id == member_id)
    });

    PolicyMemberSnapshot {
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

// ── Name helpers ────────────────────────────────────────────────────

fn inbound_protocol_name(protocol: &InboundProtocolConfig) -> &'static str {
    match protocol {
        InboundProtocolConfig::Socks5 { .. } => "socks5",
        InboundProtocolConfig::HttpConnect => "http_connect",
        InboundProtocolConfig::Mixed { .. } => "mixed",
        InboundProtocolConfig::Vless { .. } => "vless",
        InboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
        InboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
        InboundProtocolConfig::Trojan { .. } => "trojan",
        InboundProtocolConfig::Vmess { .. } => "vmess",
        InboundProtocolConfig::Direct { .. } => "direct",
        InboundProtocolConfig::Mieru { .. } => "mieru",
    }
}

fn protocol_name(protocol: ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::Socks5 => "socks5",
        ProtocolType::HttpConnect => "http_connect",
        ProtocolType::Vless => "vless",
        ProtocolType::Hysteria2 => "hysteria2",
        ProtocolType::Shadowsocks => "shadowsocks",
        ProtocolType::Trojan => "trojan",
        ProtocolType::Vmess => "vmess",
        ProtocolType::Mieru => "mieru",
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
