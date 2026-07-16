//! Managed datagram flow facade.
//!
//! The root stays as a facade so flow dispatch logic and the handler trait
//! surface do not regrow into one mixed implementation bucket.

mod dispatch;
mod handler;
