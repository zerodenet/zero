use zero_config::{ClientTlsConfig, RealityConfig, WebSocketConfig};

use super::groups::OutboundGroupStateStore;
use super::plan::{EnginePlan, OutboundTarget, TargetId, TargetKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedLeafOutbound<'a> {
    Direct {
        tag: Option<&'a str>,
    },
    Block {
        tag: Option<&'a str>,
    },
    Socks5 {
        tag: &'a str,
        server: &'a str,
        port: u16,
        username: Option<&'a str>,
        password: Option<&'a str>,
    },
    Vless {
        tag: &'a str,
        server: &'a str,
        port: u16,
        id: &'a str,
        flow: Option<&'a str>,
        mux_concurrency: Option<u32>,
        mux_idle_timeout_secs: Option<u64>,
        tls: Option<&'a ClientTlsConfig>,
        reality: Option<&'a RealityConfig>,
        ws: Option<&'a WebSocketConfig>,
        grpc: Option<&'a zero_config::GrpcConfig>,
        h2: Option<&'a zero_config::H2Config>,
        http_upgrade: Option<&'a zero_config::HttpUpgradeConfig>,
        quic: Option<&'a zero_config::QuicConfig>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedOutbound<'a> {
    Single(ResolvedLeafOutbound<'a>),
    Fallback {
        candidates: Vec<ResolvedLeafOutbound<'a>>,
    },
}

pub(crate) fn resolve_target_id<'a>(
    plan: &'a EnginePlan,
    outbound_group_state: &OutboundGroupStateStore,
    target_id: TargetId,
) -> Option<ResolvedOutbound<'a>> {
    let mut stack = Vec::new();
    resolve_target_inner(plan, outbound_group_state, target_id, &mut stack)
}

pub(crate) fn resolve_target_chains(
    plan: &EnginePlan,
    outbound_group_state: &OutboundGroupStateStore,
    target_id: TargetId,
) -> Vec<Vec<TargetId>> {
    let mut stack = Vec::new();
    resolve_target_chains_inner(plan, outbound_group_state, target_id, &mut stack)
}

fn resolve_target_inner<'a>(
    plan: &'a EnginePlan,
    outbound_group_state: &OutboundGroupStateStore,
    target_id: TargetId,
    stack: &mut Vec<TargetId>,
) -> Option<ResolvedOutbound<'a>> {
    if stack.contains(&target_id) {
        return None;
    }
    stack.push(target_id);

    let target = plan.target(target_id)?;
    let resolved = match target.kind() {
        TargetKind::Outbound(outbound) => Some(ResolvedOutbound::Single(resolve_leaf_outbound(
            target.tag(),
            outbound,
        ))),
        TargetKind::Selector(selector) => {
            let selected = outbound_group_state
                .selector_selected_target(target_id)
                .unwrap_or_else(|| selector.initial_member());
            resolve_target_inner(plan, outbound_group_state, selected, stack)
        }
        TargetKind::Fallback(fallback) => {
            let mut candidates = Vec::new();
            for &member_id in fallback.members() {
                let resolved = resolve_target_inner(plan, outbound_group_state, member_id, stack)?;
                append_candidates(&mut candidates, resolved);
            }

            Some(ResolvedOutbound::Fallback { candidates })
        }
        TargetKind::UrlTest(urltest) => {
            let selected = outbound_group_state
                .selected_target(target_id)
                .unwrap_or_else(|| urltest.initial_member());
            resolve_target_inner(plan, outbound_group_state, selected, stack)
        }
    };

    stack.pop();
    resolved
}

fn resolve_leaf_outbound<'a>(
    tag: &'a str,
    outbound: &'a OutboundTarget,
) -> ResolvedLeafOutbound<'a> {
    match outbound {
        OutboundTarget::Direct => ResolvedLeafOutbound::Direct { tag: Some(tag) },
        OutboundTarget::Block => ResolvedLeafOutbound::Block { tag: Some(tag) },
        OutboundTarget::Socks5 {
            server,
            port,
            username,
            password,
        } => ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port: *port,
            username: username.as_deref(),
            password: password.as_deref(),
        },
        OutboundTarget::Vless {
            server,
            port,
            id,
            flow,
            mux_concurrency,
            mux_idle_timeout_secs,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            quic,
        } => ResolvedLeafOutbound::Vless {
            tag,
            server,
            port: *port,
            id,
            flow: flow.as_deref(),
            mux_concurrency: *mux_concurrency,
            mux_idle_timeout_secs: *mux_idle_timeout_secs,
            tls: tls.as_ref(),
            reality: reality.as_deref(),
            ws: ws.as_ref(),
            grpc: grpc.as_ref(),
            h2: h2.as_ref(),
            http_upgrade: http_upgrade.as_ref(),
            quic: quic.as_ref(),
        },
    }
}

fn append_candidates<'a>(
    candidates: &mut Vec<ResolvedLeafOutbound<'a>>,
    resolved: ResolvedOutbound<'a>,
) {
    match resolved {
        ResolvedOutbound::Single(candidate) => candidates.push(candidate),
        ResolvedOutbound::Fallback { candidates: nested } => candidates.extend(nested),
    }
}

fn resolve_target_chains_inner(
    plan: &EnginePlan,
    outbound_group_state: &OutboundGroupStateStore,
    target_id: TargetId,
    stack: &mut Vec<TargetId>,
) -> Vec<Vec<TargetId>> {
    let Some(target) = plan.target(target_id) else {
        return Vec::new();
    };

    if stack.contains(&target_id) {
        return Vec::new();
    }

    stack.push(target_id);
    let chains = match target.kind() {
        TargetKind::Outbound(_) => vec![stack.clone()],
        TargetKind::Selector(selector) => {
            let selected = outbound_group_state
                .selector_selected_target(target_id)
                .unwrap_or_else(|| selector.initial_member());
            resolve_target_chains_inner(plan, outbound_group_state, selected, stack)
        }
        TargetKind::Fallback(fallback) => fallback
            .members()
            .iter()
            .flat_map(|member_id| {
                resolve_target_chains_inner(plan, outbound_group_state, *member_id, stack)
            })
            .collect(),
        TargetKind::UrlTest(urltest) => {
            let selected = outbound_group_state
                .selected_target(target_id)
                .unwrap_or_else(|| urltest.initial_member());
            resolve_target_chains_inner(plan, outbound_group_state, selected, stack)
        }
    };
    stack.pop();
    chains
}
