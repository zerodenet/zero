use std::io;
use std::time::Duration;

use tracing::{debug, info, warn};
use zero_core::{ProtocolType, Session};
use zero_router::RouteAction;

use super::error::EngineError;

pub(crate) fn log_session_accepted(session: &Session, route_action: &RouteAction) {
    info!(
        session_id = session.id,
        inbound_tag = session.inbound_tag.as_deref().unwrap_or("-"),
        protocol = protocol_name(session.protocol),
        target = ?session.target,
        port = session.port,
        route_action = ?route_action,
        "session accepted"
    );
}

pub(crate) fn log_session_relayed(
    session: &Session,
    duration: Duration,
    bytes_from_client: u64,
    bytes_to_client: u64,
    upstream: Option<(&str, u16)>,
) {
    match upstream {
        Some((server, port)) => info!(
            session_id = session.id,
            inbound_tag = session.inbound_tag.as_deref().unwrap_or("-"),
            outbound_tag = session.outbound_tag.as_deref().unwrap_or("-"),
            protocol = protocol_name(session.protocol),
            target = ?session.target,
            port = session.port,
            upstream_server = server,
            upstream_port = port,
            duration_ms = duration.as_millis() as u64,
            bytes_from_client,
            bytes_to_client,
            "session relayed"
        ),
        None => info!(
            session_id = session.id,
            inbound_tag = session.inbound_tag.as_deref().unwrap_or("-"),
            outbound_tag = session.outbound_tag.as_deref().unwrap_or("-"),
            protocol = protocol_name(session.protocol),
            target = ?session.target,
            port = session.port,
            duration_ms = duration.as_millis() as u64,
            bytes_from_client,
            bytes_to_client,
            "session relayed"
        ),
    }
}

pub(crate) fn log_session_blocked(session: &Session, duration: Duration) {
    info!(
        session_id = session.id,
        inbound_tag = session.inbound_tag.as_deref().unwrap_or("-"),
        outbound_tag = session.outbound_tag.as_deref().unwrap_or("-"),
        protocol = protocol_name(session.protocol),
        target = ?session.target,
        port = session.port,
        duration_ms = duration.as_millis() as u64,
        "session blocked"
    );
}

pub(crate) fn log_session_failed(
    session: &Session,
    stage: &'static str,
    duration: Duration,
    error: &impl std::fmt::Display,
    upstream: Option<(&str, u16)>,
) {
    match upstream {
        Some((server, port)) => warn!(
            session_id = session.id,
            inbound_tag = session.inbound_tag.as_deref().unwrap_or("-"),
            outbound_tag = session.outbound_tag.as_deref().unwrap_or("-"),
            protocol = protocol_name(session.protocol),
            target = ?session.target,
            port = session.port,
            stage = stage,
            error = %error,
            upstream_server = server,
            upstream_port = port,
            duration_ms = duration.as_millis() as u64,
            "session failed"
        ),
        None => warn!(
            session_id = session.id,
            inbound_tag = session.inbound_tag.as_deref().unwrap_or("-"),
            outbound_tag = session.outbound_tag.as_deref().unwrap_or("-"),
            protocol = protocol_name(session.protocol),
            target = ?session.target,
            port = session.port,
            stage = stage,
            error = %error,
            duration_ms = duration.as_millis() as u64,
            "session failed"
        ),
    }
}

fn protocol_name(protocol: ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::Socks5 => "socks5",
        ProtocolType::HttpConnect => "http-connect",
        ProtocolType::Unknown => "unknown",
    }
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
