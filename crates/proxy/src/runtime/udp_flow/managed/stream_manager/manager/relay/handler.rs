//! Managed stream relay handler facade.
//!
//! The root stays as a facade so packet-flow and relay-flow handler adapters do
//! not regrow into one mixed implementation bucket.

#[cfg(feature = "managed-stream-runtime")]
mod packet;
#[cfg(feature = "managed-stream-runtime")]
mod relay;
