//! UDP flow state facade plus focused lifecycle and forwarding helpers.
//!
//! The root stays as a facade so state model, lifecycle polling, managed-flow
//! dispatch, and packet-path forwarding do not regrow into one large
//! implementation bucket.

mod context;
mod lifecycle;
mod managed;
mod model;
mod packet_path;

pub(crate) use context::UdpFlowStartContext;
#[cfg(feature = "socks5")]
pub(crate) use lifecycle::UpstreamUdpPoll;
pub(crate) use model::UdpFlowState;
