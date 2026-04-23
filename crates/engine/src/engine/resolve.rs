use super::outbound_group_state::OutboundGroupStateStore;
use super::plan::{EnginePlan, OutboundTarget, TargetId, TargetKind};

pub(crate) enum ResolvedLeafOutbound<'a> {
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
    },
}

pub(crate) enum ResolvedOutbound<'a> {
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
        OutboundTarget::Socks5 { server, port } => ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port: *port,
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
