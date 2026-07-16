//! Stream-carried inbound UDP relay facade.
//!
//! The root stays as a facade so relay orchestration, handler adaptation,
//! and client I/O recording do not collapse back into one implementation bucket.

mod handler;
mod recording;
mod relay;

pub(crate) use relay::run_mapped_protocol_stream_udp_relay;
