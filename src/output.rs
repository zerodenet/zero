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
            match &group.selected {
                Some(selected) => output.push_str(&format!(
                    "  - {} {} selected={} members={}\n",
                    group.tag,
                    group.kind,
                    selected,
                    group.outbounds.join(",")
                )),
                None => output.push_str(&format!(
                    "  - {} {} members={}\n",
                    group.tag,
                    group.kind,
                    group.outbounds.join(",")
                )),
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
