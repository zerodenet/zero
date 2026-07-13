use std::io;

use tracing::{debug, warn};
use zero_engine::EngineError;

pub(crate) const INBOUND_ACCEPT_ROUTE_STAGE: &str = "inbound_accept_route";

pub(crate) fn log_listener_connection_error(
    stage: &'static str,
    protocol: &'static str,
    inbound_tag: &str,
    remote_addr: &impl std::fmt::Debug,
    error: &EngineError,
) {
    if is_transient_disconnect(error) {
        debug!(inbound_tag, ?remote_addr, protocol, stage, error = %error,
            "connection closed before request completed");
    } else {
        warn!(inbound_tag, ?remote_addr, protocol, stage, error = %error, "connection failed");
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

#[cfg(test)]
#[path = "listener/tests.rs"]
mod tests;
