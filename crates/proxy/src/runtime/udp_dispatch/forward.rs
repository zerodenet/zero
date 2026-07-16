//! UDP flow forwarding for existing outbound connections.
//!
//! Keeps first-level category dispatch separate from common failure/result
//! normalization so this nested root stays as a facade.

mod path;
mod result;
