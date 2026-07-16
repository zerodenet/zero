//! Registered upstream control facade.
//!
//! The root stays as a facade so start-flow logic and upstream close/logging
//! helpers do not regrow into one mixed implementation bucket.

mod close;
mod start;

pub(crate) use close::{close_registered_dropped_upstream, close_registered_idle_upstream};
pub(crate) use start::start_registered_upstream_flow;
