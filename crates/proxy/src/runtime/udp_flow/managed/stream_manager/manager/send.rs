//! Managed stream send facade.
//!
//! The root stays as a facade so direct-send execution and existing-request
//! projection do not regrow into one mixed implementation bucket.

mod dispatch;
mod existing;
