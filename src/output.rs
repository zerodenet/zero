use zero_engine::{AddressExport, EngineStatusExport};

pub fn render_status(status: &EngineStatusExport) -> String {
    let mut output = String::new();

    output.push_str("Engine Status\n");
    output.push_str("config:\n");
    output.push_str(&format!("  inbounds: {}\n", status.config.inbounds.len()));
    output.push_str(&format!("  outbounds: {}\n", status.config.outbounds.len()));
    output.push_str(&format!("  rules: {}\n", status.config.rule_count));
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

    if !status.runtime.active_sessions.is_empty() {
        output.push_str("active_sessions:\n");
        for session in &status.runtime.active_sessions {
            output.push_str(&format!(
                "  - #{} {} {}:{} inbound={} outbound={}\n",
                session.id,
                session.protocol,
                render_address(&session.target),
                session.port,
                session.inbound_tag.as_deref().unwrap_or("-"),
                session.outbound_tag.as_deref().unwrap_or("-")
            ));
        }
    }

    output
}

fn render_address(address: &AddressExport) -> &str {
    &address.value
}
