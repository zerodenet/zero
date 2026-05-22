//! VLESS outbound — re-exports from runtime orchestration layer.
//!
//! The VLESS UDP types and manager moved to `crate::runtime::vless_udp` so that
//! inbound handlers can import them without depending on the outbound module.

// Re-exports for backward compatibility — remove once all callers migrate.
// pub(crate) use crate::runtime::vless_udp::{
//     establish_vless_udp_upstream, VlessUdpOutboundManager, VlessUdpTransport, VlessUdpUpstream,
// };
