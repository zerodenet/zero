//! Inbound route-runtime facade plus focused service/runtime builders.
//!
//! The root stays as a facade so shared ingress services, per-connection route
//! runtimes, listener runtimes, and mux-substream helpers do not regrow into
//! one large implementation bucket.

mod listener;
#[cfg(any(feature = "vless", feature = "vmess"))]
mod mux;
mod route;
mod shared;

pub(crate) use listener::{InboundListenerRuntime, InboundListenerRuntimeFactory};
#[cfg(any(feature = "vless", feature = "vmess"))]
pub(crate) use mux::MuxSubstreamRuntime;
pub(crate) use route::{InboundRouteRuntime, InboundRouteRuntimeFactory};
pub(crate) use shared::SharedIngressRuntimeServices;
