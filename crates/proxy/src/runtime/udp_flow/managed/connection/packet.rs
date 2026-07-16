//! Managed packet UDP connection facade.
//!
//! The root stays as a facade so the packet sender contract, flow adaptation,
//! response-bridge connection shell, and construction helpers do not regrow
//! into one mixed implementation bucket.

mod build;
mod connection;
mod flow;
mod sender;

pub(crate) use build::managed_packet_udp_connection_from_flow;
