//! Upstream association lifecycle facade.
//!
//! The root stays as a facade so send/establish flows and close/drop/idle
//! lifecycle handling do not regrow into one mixed implementation bucket.

mod close;
mod send;
