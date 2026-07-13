//! TCP connection dispatch: routing pipeline and neutral relay orchestration.
//!
//! The root stays as a facade so TCP leaf dispatch and relay-chain glue can be
//! split without turning this file back into a catch-all implementation bucket.

mod leaf;
pub(crate) mod operation;
mod relay;
