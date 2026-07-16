//! Managed stream relay facade.
//!
//! The root stays as a facade so internal relay-send execution and shared
//! handler trait adapters do not regrow into one mixed implementation bucket.

mod dispatch;
mod handler;
