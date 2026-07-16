//! Registered upstream handler facade.
//!
//! The root stays as a facade so the runtime holder, handler dispatch, and box
//! constructor do not regrow into one mixed implementation bucket.

mod build;
mod dispatch;
mod model;

pub(crate) use build::boxed_registered_upstream_handler;
