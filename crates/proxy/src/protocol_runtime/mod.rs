//! Protocol-specific runtime state.
//!
//! Generic `runtime` owns lifecycle, routing, pipes, and dispatch. Protocol
//! pools and UDP state machines live here so new protocol dependencies do not
//! drift into generic runtime modules.

pub(crate) mod socks5_udp_associate;
pub(crate) mod udp;
