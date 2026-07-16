//! Managed datagram socket facade.
//!
//! The root stays as a facade so socket dispatch logic and the handler trait
//! surface do not regrow into one mixed implementation bucket.

mod dispatch;
mod handler;
