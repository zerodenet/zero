//! Protocol-specific runtime state.
//!
//! Generic `runtime` owns lifecycle, routing, pipes, and dispatch. Protocol
//! pools and UDP state machines live here so new protocol dependencies do not
//! drift into generic runtime modules.

pub(crate) mod socks5_udp;
pub(crate) mod socks5_udp_associate;
pub(crate) mod udp;
pub(crate) mod vless_mux_pool;
pub(crate) mod vless_udp;

#[cfg(feature = "vmess")]
pub(crate) mod vmess_mux_pool;
#[cfg(feature = "vmess")]
pub(crate) mod vmess_udp;
