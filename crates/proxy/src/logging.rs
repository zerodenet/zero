use std::io;
use std::time::Duration;

use tracing::{debug, info, warn};
use zero_core::{Network, ProtocolType, Session};

use zero_engine::{CompletedSessionRecord, EngineError};

pub(crate) fn log_session_accepted(
    session: &Session,
    route_action: &impl std::fmt::Debug,
    mode: &str,
) {
    info!(
        session_id = session.id,
        inbound_tag = session.inbound_tag.as_deref().unwrap_or("-"),
        protocol = protocol_name(session.protocol),
        network = network_name(session.network),
        mode = mode,
        target = ?session.target,
        port = session.port,
        route_action = ?route_action,
        "session accepted"
    );
}

pub(crate) fn log_session_finished(record: &CompletedSessionRecord, upstream: Option<(&str, u16)>) {
    match upstream {
        Some((server, port)) => info!(
            session_id = record.id,
            inbound_tag = record.inbound_tag.as_deref().unwrap_or("-"),
            outbound_tag = record.outbound_tag.as_deref().unwrap_or("-"),
            protocol = protocol_name(record.protocol),
            network = network_name(record.network),
            mode = record.mode.as_str(),
            target = ?record.target,
            port = record.port,
            upstream_server = server,
            upstream_port = port,
            outcome = record.outcome.kind(),
            duration_ms = record.duration_ms,
            bytes_up = record.bytes_up,
            bytes_down = record.bytes_down,
            inbound_rx_bytes = record.inbound_rx_bytes,
            inbound_tx_bytes = record.inbound_tx_bytes,
            outbound_rx_bytes = record.outbound_rx_bytes,
            outbound_tx_bytes = record.outbound_tx_bytes,
            "session finished"
        ),
        None => info!(
            session_id = record.id,
            inbound_tag = record.inbound_tag.as_deref().unwrap_or("-"),
            outbound_tag = record.outbound_tag.as_deref().unwrap_or("-"),
            protocol = protocol_name(record.protocol),
            network = network_name(record.network),
            mode = record.mode.as_str(),
            target = ?record.target,
            port = record.port,
            outcome = record.outcome.kind(),
            duration_ms = record.duration_ms,
            bytes_up = record.bytes_up,
            bytes_down = record.bytes_down,
            inbound_rx_bytes = record.inbound_rx_bytes,
            inbound_tx_bytes = record.inbound_tx_bytes,
            outbound_rx_bytes = record.outbound_rx_bytes,
            outbound_tx_bytes = record.outbound_tx_bytes,
            "session finished"
        ),
    }
}

pub(crate) fn log_session_failed(
    session: &Session,
    record: Option<&CompletedSessionRecord>,
    stage: &'static str,
    duration: Duration,
    error: &impl std::fmt::Display,
    upstream: Option<(&str, u16)>,
) {
    let mode = record.map(|item| item.mode.as_str()).unwrap_or("-");
    let duration_ms = record
        .map(|item| item.duration_ms)
        .unwrap_or(duration.as_millis() as u64);
    let bytes_up = record.map(|item| item.bytes_up).unwrap_or(0);
    let bytes_down = record.map(|item| item.bytes_down).unwrap_or(0);
    let inbound_rx_bytes = record.map(|item| item.inbound_rx_bytes).unwrap_or(0);
    let inbound_tx_bytes = record.map(|item| item.inbound_tx_bytes).unwrap_or(0);
    let outbound_rx_bytes = record.map(|item| item.outbound_rx_bytes).unwrap_or(0);
    let outbound_tx_bytes = record.map(|item| item.outbound_tx_bytes).unwrap_or(0);

    match upstream {
        Some((server, port)) => warn!(
            session_id = session.id,
            inbound_tag = session.inbound_tag.as_deref().unwrap_or("-"),
            outbound_tag = session.outbound_tag.as_deref().unwrap_or("-"),
            protocol = protocol_name(session.protocol),
            network = network_name(session.network),
            mode = mode,
            target = ?session.target,
            port = session.port,
            stage = stage,
            error = %error,
            upstream_server = server,
            upstream_port = port,
            duration_ms = duration_ms,
            bytes_up = bytes_up,
            bytes_down = bytes_down,
            inbound_rx_bytes = inbound_rx_bytes,
            inbound_tx_bytes = inbound_tx_bytes,
            outbound_rx_bytes = outbound_rx_bytes,
            outbound_tx_bytes = outbound_tx_bytes,
            "session failed"
        ),
        None => warn!(
            session_id = session.id,
            inbound_tag = session.inbound_tag.as_deref().unwrap_or("-"),
            outbound_tag = session.outbound_tag.as_deref().unwrap_or("-"),
            protocol = protocol_name(session.protocol),
            network = network_name(session.network),
            mode = mode,
            target = ?session.target,
            port = session.port,
            stage = stage,
            error = %error,
            duration_ms = duration_ms,
            bytes_up = bytes_up,
            bytes_down = bytes_down,
            inbound_rx_bytes = inbound_rx_bytes,
            inbound_tx_bytes = inbound_tx_bytes,
            outbound_rx_bytes = outbound_rx_bytes,
            outbound_tx_bytes = outbound_tx_bytes,
            "session failed"
        ),
    }
}

fn protocol_name(protocol: ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::Socks5 => "socks5",
        ProtocolType::HttpConnect => "http-connect",
        ProtocolType::Vless => "vless",
        ProtocolType::Unknown => "unknown",
    }
}

fn network_name(network: Network) -> &'static str {
    match network {
        Network::Tcp => "tcp",
        Network::Udp => "udp",
    }
}

pub(crate) fn log_urltest_group_target_changed(
    group_tag: &str,
    previous: Option<&str>,
    selected: &str,
    latency_ms: Option<u64>,
) {
    match previous {
        Some(previous) if previous == selected => debug!(
            group_kind = "urltest",
            group_tag = group_tag,
            selected = selected,
            latency_ms = latency_ms,
            "outbound group probe refreshed"
        ),
        Some(previous) => info!(
            group_kind = "urltest",
            group_tag = group_tag,
            previous = previous,
            selected = selected,
            latency_ms = latency_ms,
            "outbound group target changed"
        ),
        None => info!(
            group_kind = "urltest",
            group_tag = group_tag,
            selected = selected,
            latency_ms = latency_ms,
            "outbound group target initialized"
        ),
    }
}

#[cfg(feature = "inbound-socks5")]
pub(crate) fn log_udp_upstream_association_created(
    inbound_tag: &str,
    outbound_tag: &str,
    server: &str,
    port: u16,
    idle_timeout: Duration,
) {
    info!(
        inbound_tag = inbound_tag,
        outbound_tag = outbound_tag,
        protocol = "socks5-udp",
        upstream_server = server,
        upstream_port = port,
        idle_timeout_seconds = idle_timeout.as_secs(),
        "created upstream UDP association"
    );
}

#[cfg(feature = "inbound-socks5")]
pub(crate) fn log_udp_upstream_association_reused(
    inbound_tag: &str,
    outbound_tag: &str,
    server: &str,
    port: u16,
) {
    debug!(
        inbound_tag = inbound_tag,
        outbound_tag = outbound_tag,
        protocol = "socks5-udp",
        upstream_server = server,
        upstream_port = port,
        "reused upstream UDP association"
    );
}

#[cfg(feature = "inbound-socks5")]
pub(crate) fn log_udp_upstream_association_idle_timeout(
    inbound_tag: &str,
    outbound_tag: &str,
    server: &str,
    port: u16,
    idle_timeout: Duration,
) {
    info!(
        inbound_tag = inbound_tag,
        outbound_tag = outbound_tag,
        protocol = "socks5-udp",
        upstream_server = server,
        upstream_port = port,
        idle_timeout_seconds = idle_timeout.as_secs(),
        "closed idle upstream UDP association"
    );
}

#[cfg(feature = "inbound-socks5")]
pub(crate) fn log_udp_upstream_association_dropped(
    inbound_tag: &str,
    outbound_tag: &str,
    server: &str,
    port: u16,
    error: &impl std::fmt::Display,
) {
    warn!(
        inbound_tag = inbound_tag,
        outbound_tag = outbound_tag,
        protocol = "socks5-udp",
        upstream_server = server,
        upstream_port = port,
        error = %error,
        "dropped upstream UDP association"
    );
}

pub(crate) fn log_listener_connection_error(
    protocol: &'static str,
    inbound_tag: &str,
    remote_addr: &impl std::fmt::Debug,
    error: &EngineError,
) {
    if is_transient_disconnect(error) {
        debug!(
            inbound_tag = inbound_tag,
            ?remote_addr,
            protocol = protocol,
            error = %error,
            "connection closed before request completed"
        );
    } else {
        warn!(
            inbound_tag = inbound_tag,
            ?remote_addr,
            protocol = protocol,
            error = %error,
            "connection failed"
        );
    }
}

fn is_transient_disconnect(error: &EngineError) -> bool {
    match error {
        EngineError::Io(source) => matches!(
            source.kind(),
            io::ErrorKind::UnexpectedEof
                | io::ErrorKind::ConnectionAborted
                | io::ErrorKind::ConnectionReset
                | io::ErrorKind::BrokenPipe
                | io::ErrorKind::NotConnected
        ),
        EngineError::Core(zero_core::Error::Io(message)) => {
            message.contains("unexpected EOF") || message.contains("failed to read")
        }
        _ => false,
    }
}
