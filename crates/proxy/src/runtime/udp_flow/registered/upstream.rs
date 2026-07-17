mod contract;
#[cfg(feature = "udp-runtime")]
mod runtime;
#[cfg(feature = "udp-runtime")]
mod state;

pub(crate) use contract::UpstreamAssociationHandler;
pub(crate) use contract::UpstreamAssociationSend;
#[cfg(feature = "udp-runtime")]
pub(crate) use contract::UpstreamUdpHandlers;
#[cfg(feature = "socks5")]
pub(crate) use contract::{
    UpstreamAssociationCloseReason, UpstreamAssociationStages, UpstreamAssociationTarget,
    UpstreamAssociationTransport,
};
#[cfg(feature = "socks5")]
pub(crate) use runtime::boxed_registered_upstream_handler;
#[cfg(all(test, feature = "socks5"))]
pub(crate) use runtime::UpstreamAssociationRuntime;
#[cfg(feature = "udp-runtime")]
pub(super) use state::handlers::UpstreamAssociationState;
#[cfg(all(test, feature = "socks5"))]
pub(crate) use state::TrackedUpstreamAssociationState;

#[cfg(all(test, feature = "socks5"))]
mod tests;
