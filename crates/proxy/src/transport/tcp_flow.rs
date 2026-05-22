//! Shared TCP flow utilities.
//!
//! The TCP session lifecycle (handle_tcp_session, relay_tcp_session, response sending)
//! lived here but has been replaced by `crate::runtime::inbound_protocol::serve_inbound`.

use zero_engine::EngineError;

/// Detect the synthetic "blocked" error produced by `extract_tcp_stream`
/// when an outbound resolves to `Block`.
pub(crate) fn is_block_error(error: &EngineError) -> bool {
    matches!(
        error,
        EngineError::Io(e)
            if e.kind() == std::io::ErrorKind::ConnectionRefused
                && e.to_string() == "blocked"
    )
}
