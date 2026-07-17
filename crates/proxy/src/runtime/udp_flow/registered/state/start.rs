//! Registered UDP state start facade.
//!
//! The root stays as a facade so upstream-association handoff, managed-flow
//! handoff, and unhandled-start error construction do not regrow into one
//! implementation bucket.

mod error;
mod managed;
#[cfg(feature = "upstream-association-runtime")]
mod upstream;
