//! Managed cache insert facade.
//!
//! The root stays as a facade so pre-sent cache reuse, establish-on-miss, and
//! relay insert-and-send paths do not regrow into one mixed implementation
//! bucket.

mod establish;
#[cfg(feature = "managed-datagram-runtime")]
mod pre_sent;
#[cfg(feature = "managed-stream-runtime")]
mod relay;
