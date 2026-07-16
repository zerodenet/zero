//! UDP outbound flow model plus focused projection helpers.
//!
//! The root stays as a facade so flow variants, index keys, and completion
//! helpers do not regrow into one large implementation bucket.

mod model;
mod projection;

#[allow(unused_imports)]
pub(crate) use model::{ManagedUdpFlowRef, UdpFlowOutbound, UdpFlowUpstream};
#[allow(unused_imports)]
pub(in crate::runtime::udp_flow) use model::{UdpFlowCompletion, UdpFlowIndexKeys};
