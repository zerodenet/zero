//! UDP association lifecycle facade.
//!
//! The root stays as a facade so relay orchestration, idle handling, and
//! upstream or chain response handling do not regrow into one large
//! implementation bucket.

mod idle;
mod relay;
mod response;

pub(crate) use relay::run_udp_association_loop;
