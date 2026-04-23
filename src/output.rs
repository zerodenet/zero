use zero_engine::{AddressExport, EngineStatusExport};

pub fn render_status(status: &EngineStatusExport) -> String {
    let mut output = String::new();

    output.push_str("Engine Status\n");
    output.push_str("config:\n");
    output.push_str(&format!("  mode: {}\n", status.config.mode.kind));
    output.push_str(&format!("  inbounds: {}\n", status.config.inbounds.len()));
    output.push_str(&format!("  outbounds: {}\n", status.config.outbounds.len()));
    output.push_str(&format!(
        "  outbound_groups: {}\n",
        status.config.outbound_groups.len()
    ));
    output.push_str(&format!("  rules: {}\n", status.config.rule_count));

    if let Some(outbound) = &status.config.mode.outbound {
        output.push_str(&format!("  mode_outbound: {}\n", outbound));
    }

    output.push_str("runtime:\n");
    output.push_str(&format!(
        "  sessions: active={} total={} completed={} failed={} blocked={} direct={} chained={}\n",
        status.runtime.stats.active_sessions,
        status.runtime.stats.total_started,
        status.runtime.stats.completed_sessions,
        status.runtime.stats.failed_sessions,
        status.runtime.stats.blocked_sessions,
        status.runtime.stats.direct_sessions,
        status.runtime.stats.chained_sessions
    ));
    output.push_str(&format!(
        "  udp_upstream: timeout={}s active={} created={} reused={} closed={} idle_timeouts={} dropped={} assoc_failures={} send_failures={} recv_failures={} packets_sent={} packets_received={}\n",
        status.runtime.udp_upstream_idle_timeout_seconds,
        status.runtime.stats.udp_upstream.active_associations,
        status.runtime.stats.udp_upstream.created_associations,
        status.runtime.stats.udp_upstream.reused_associations,
        status.runtime.stats.udp_upstream.closed_associations,
        status.runtime.stats.udp_upstream.idle_timeouts,
        status.runtime.stats.udp_upstream.dropped_associations,
        status.runtime.stats.udp_upstream.failed_association_attempts,
        status.runtime.stats.udp_upstream.send_failures,
        status.runtime.stats.udp_upstream.recv_failures,
        status.runtime.stats.udp_upstream.packets_sent,
        status.runtime.stats.udp_upstream.packets_received,
    ));

    if !status.config.inbounds.is_empty() {
        output.push_str("listeners:\n");
        for inbound in &status.config.inbounds {
            output.push_str(&format!(
                "  - {} {}://{}:{}\n",
                inbound.tag, inbound.protocol, inbound.listen_address, inbound.listen_port
            ));
        }
    }

    if !status.config.outbounds.is_empty() {
        output.push_str("outbounds:\n");
        for outbound in &status.config.outbounds {
            match (&outbound.server, outbound.port) {
                (Some(server), Some(port)) => output.push_str(&format!(
                    "  - {} {} {}:{}\n",
                    outbound.tag, outbound.protocol, server, port
                )),
                _ => output.push_str(&format!("  - {} {}\n", outbound.tag, outbound.protocol)),
            }
        }
    }

    if !status.config.outbound_groups.is_empty() {
        output.push_str("outbound_groups:\n");
        for group in &status.config.outbound_groups {
            match (
                &group.selected,
                group.latency_ms,
                group.last_checked_unix_ms,
            ) {
                (Some(selected), Some(latency_ms), Some(last_checked_unix_ms)) => {
                    output.push_str(&format!(
                        "  - {} {} selected={} latency={}ms checked_at={} members={} effective={}\n",
                        group.tag,
                        group.kind,
                        selected,
                        latency_ms,
                        last_checked_unix_ms,
                        group.outbounds.join(","),
                        render_chains(&group.effective_chains),
                    ))
                }
                (Some(selected), _, _) => output.push_str(&format!(
                    "  - {} {} selected={} members={} effective={}\n",
                    group.tag,
                    group.kind,
                    selected,
                    group.outbounds.join(","),
                    render_chains(&group.effective_chains),
                )),
                (None, _, _) => output.push_str(&format!(
                    "  - {} {} members={} effective={}\n",
                    group.tag,
                    group.kind,
                    group.outbounds.join(","),
                    render_chains(&group.effective_chains),
                )),
            }

            for member in &group.urltest_members {
                match (
                    member.healthy,
                    member.latency_ms,
                    member.last_error.as_deref(),
                ) {
                    (true, Some(latency_ms), _) => output.push_str(&format!(
                        "    probe {} healthy latency={}ms checked_at={} effective={}\n",
                        member.member_tag,
                        latency_ms,
                        member.last_checked_unix_ms.unwrap_or_default(),
                        render_chains(&member.effective_chains),
                    )),
                    (_, _, Some(error)) => output.push_str(&format!(
                        "    probe {} unhealthy error={} checked_at={} effective={}\n",
                        member.member_tag,
                        error,
                        member.last_checked_unix_ms.unwrap_or_default(),
                        render_chains(&member.effective_chains),
                    )),
                    _ => output.push_str(&format!(
                        "    probe {} pending effective={}\n",
                        member.member_tag,
                        render_chains(&member.effective_chains),
                    )),
                }
            }
        }
    }

    if !status.runtime.active_sessions.is_empty() {
        output.push_str("active_sessions:\n");
        for session in &status.runtime.active_sessions {
            output.push_str(&format!(
                "  - #{} {} {} {}:{} inbound={} outbound={} up={} down={} throughput_up={}/s throughput_down={}/s\n",
                session.id,
                session.network,
                session.protocol,
                render_address(&session.target),
                session.port,
                session.inbound_tag.as_deref().unwrap_or("-"),
                session.outbound_tag.as_deref().unwrap_or("-"),
                session.bytes_up,
                session.bytes_down,
                session.throughput_up_bps,
                session.throughput_down_bps,
            ));
        }
    }

    if !status.runtime.recent_completed_sessions.is_empty() {
        output.push_str("recent_completed_sessions:\n");
        for session in status.runtime.recent_completed_sessions.iter().take(10) {
            output.push_str(&format!(
                "  - #{} {} {} {}:{} inbound={} outbound={} outcome={} up={} down={} duration={}ms\n",
                session.id,
                session.network,
                session.protocol,
                render_address(&session.target),
                session.port,
                session.inbound_tag.as_deref().unwrap_or("-"),
                session.outbound_tag.as_deref().unwrap_or("-"),
                session.outcome,
                session.bytes_up,
                session.bytes_down,
                session.duration_ms,
            ));
        }
    }

    output
}

fn render_address(address: &AddressExport) -> &str {
    &address.value
}

fn render_chains(chains: &[Vec<String>]) -> String {
    if chains.is_empty() {
        return "-".to_owned();
    }

    chains
        .iter()
        .map(|chain| chain.join("->"))
        .collect::<Vec<_>>()
        .join(" | ")
}
