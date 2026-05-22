//! Direct outbound — re-exports from runtime orchestration layer.
//!
//! The resolve/send helpers moved to `crate::runtime::udp_helpers` so that
//! inbound handlers can import them without depending on the outbound module.

// Re-exports for backward compatibility — remove once all callers migrate.
// pub(crate) use crate::runtime::udp_helpers::{resolve_udp_target, send_direct_udp_packet};
