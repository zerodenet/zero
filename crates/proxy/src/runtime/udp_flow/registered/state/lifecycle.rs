//! Registered UDP state lifecycle facade.
//!
//! The root stays as a facade so state construction, managed-flow resume
//! tracking, and upstream association accessors do not regrow into one
//! implementation bucket.

mod build;
mod managed;
#[cfg(feature = "socks5")]
mod upstream;
