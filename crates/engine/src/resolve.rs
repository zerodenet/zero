use rand::Rng;
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
        split_http: Option<&'a zero_config::SplitHttpConfig>,
        quic: Option<&'a zero_config::QuicConfig>,
    },
    Hysteria2 {
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        insecure: bool,
    },
    Shadowsocks {
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        cipher: &'a str,
    },
    Trojan {
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        sni: Option<&'a str>,
        insecure: bool,
        client_fingerprint: Option<&'a str>,
    },
    Vmess {
        tag: &'a str,
        server: &'a str,
        port: u16,
        id: &'a str,
        cipher: &'a str,
        tls: Option<&'a ClientTlsConfig>,
        ws: Option<&'a zero_config::WebSocketConfig>,
        grpc: Option<&'a zero_config::GrpcConfig>,
    },
    Mieru {
        tag: &'a str,
        server: &'a str,
        port: u16,
        username: &'a str,
        password: &'a str,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedOutbound<'a> {
    Single(ResolvedLeafOutbound<'a>),
    Fallback {
        candidates: Vec<ResolvedLeafOutbound<'a>>,
    },
    /// Chain of proxies connected sequentially.  Each hop's connection
    /// is established through the previous hop's TCP stream.
    Relay {
        chain: Vec<ResolvedLeafOutbound<'a>>,
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
        TargetKind::Relay(relay) => {
            let mut chain = Vec::with_capacity(relay.chain().len());
            for &member_id in relay.chain() {
                let resolved = resolve_target_inner(plan, outbound_group_state, member_id, stack)?;
                match resolved {
                    ResolvedOutbound::Single(leaf) => chain.push(leaf),
                    _ => return None,
                }
            }
            Some(ResolvedOutbound::Relay { chain })
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
        TargetKind::LoadBalance(lb) => {
            let member_count = lb.members().len();
            let index = match lb.strategy() {
                zero_config::LoadBalanceStrategy::RoundRobin => {
                    outbound_group_state.loadbalance_next_pick(target_id, member_count)
                }
                zero_config::LoadBalanceStrategy::Random => {
                    rand::rng().random_range(0..member_count)
                }
            };

            // Picked member first, remaining members follow in original order.
            let mut ordered = Vec::with_capacity(member_count);
            ordered.push(lb.members()[index]);
            ordered.extend(
                lb.members()
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != index)
                    .map(|(_, &member_id)| member_id),
            );

            let mut candidates = Vec::new();
            for &member_id in &ordered {
                let resolved = resolve_target_inner(plan, outbound_group_state, member_id, stack)?;
                append_candidates(&mut candidates, resolved);
            }

            Some(ResolvedOutbound::Fallback { candidates })
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
            split_http,
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
            split_http: split_http.as_ref(),
            quic: quic.as_ref(),
        },
        OutboundTarget::Hysteria2 {
            server,
            port,
            password,
            insecure,
        } => ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port: *port,
            password,
            insecure: *insecure,
        },
        OutboundTarget::Shadowsocks {
            server,
            port,
            password,
            cipher,
        } => ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port: *port,
            password,
            cipher,
        },
        OutboundTarget::Trojan {
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } => ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port: *port,
            password,
            sni: sni.as_deref(),
            insecure: *insecure,
            client_fingerprint: client_fingerprint.as_deref(),
        },
        OutboundTarget::Vmess {
            server,
            port,
            id,
            cipher,
            tls,
            ws,
            grpc,
        } => ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port: *port,
            id,
            cipher,
            tls: tls.as_ref(),
            ws: ws.as_ref(),
            grpc: grpc.as_ref(),
        },
        OutboundTarget::Mieru {
            server,
            port,
            username,
            password,
        } => ResolvedLeafOutbound::Mieru {
            tag,
            server,
            port: *port,
            username,
            password,
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
        ResolvedOutbound::Relay { .. } => {} // relay inside fallback is not meaningful
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
        TargetKind::Relay(relay) => {
            // Relay chains go through all proxies in order.
            let mut full_chain = stack.clone();
            for &member_id in relay.chain() {
                full_chain.push(member_id);
            }
            vec![full_chain]
        }
        TargetKind::UrlTest(urltest) => {
            let selected = outbound_group_state
                .selected_target(target_id)
                .unwrap_or_else(|| urltest.initial_member());
            resolve_target_chains_inner(plan, outbound_group_state, selected, stack)
        }
        TargetKind::LoadBalance(lb) => lb
            .members()
            .iter()
            .flat_map(|member_id| {
                resolve_target_chains_inner(plan, outbound_group_state, *member_id, stack)
            })
            .collect(),
    };
    stack.pop();
    chains
}
