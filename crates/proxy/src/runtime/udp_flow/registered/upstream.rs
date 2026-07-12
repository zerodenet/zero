mod contract;
mod runtime;
mod state;

pub(crate) use contract::{
    UpstreamAssociationCloseReason, UpstreamAssociationHandler, UpstreamAssociationStages,
    UpstreamAssociationTarget, UpstreamAssociationTransport, UpstreamUdpHandlers,
};
pub(crate) use runtime::boxed_registered_upstream_handler;
#[cfg(test)]
pub(crate) use runtime::UpstreamAssociationRuntime;
pub(super) use state::handlers::UpstreamAssociationState;
#[cfg(test)]
pub(crate) use state::TrackedUpstreamAssociationState;

#[cfg(test)]
mod tests;
