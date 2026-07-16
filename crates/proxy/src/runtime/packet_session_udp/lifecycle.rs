//! Packet-session UDP relay lifecycle facade.
//!
//! The root stays as a facade so relay orchestration, failure handling, and
//! feature-specific event loops do not regrow into one large implementation
//! bucket.

mod failure;
mod relay;
#[cfg(feature = "socks5")]
mod with_upstream;
#[cfg(not(feature = "socks5"))]
mod without_upstream;

pub(crate) use relay::run_packet_session_udp_relay;
