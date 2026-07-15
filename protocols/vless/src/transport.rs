//! Unified VLESS transport builder.
//!
//! Wraps a raw TCP socket with the configured VLESS transport layer
//! (TLS / Reality / WebSocket / gRPC / H2), dispatching to the correct
//! connect function for every valid combination.

mod inbound;
mod leaf;
mod managed_udp;
mod outbound;
mod profile;

pub use inbound::{VlessInboundBindPlan, VlessInboundListenerRequest};
pub use leaf::VlessOutboundLeaf;
pub use profile::{VlessQuicBindProfile, VlessQuicClientProfile, VlessRealityClientProfile};
