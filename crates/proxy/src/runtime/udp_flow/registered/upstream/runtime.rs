#[cfg(feature = "upstream-association-runtime")]
mod association;
#[cfg(feature = "upstream-association-runtime")]
mod control;
#[cfg(feature = "upstream-association-runtime")]
mod handler;
mod mismatch;

#[cfg(all(test, feature = "upstream-association-runtime"))]
pub(crate) use association::UpstreamAssociationRuntime;
#[cfg(feature = "upstream-association-runtime")]
pub(crate) use handler::boxed_registered_upstream_handler;
pub(crate) use mismatch::upstream_flow_mismatch;
