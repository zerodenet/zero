//! Unified VLESS transport builder.
//!
//! Wraps a raw TCP socket with the configured VLESS transport layer
//! (TLS / Reality / WebSocket / gRPC / H2), dispatching to the correct
//! connect function for every valid combination.

mod bridge;
mod inbound;
mod leaf;
mod managed_udp;
mod outbound;
mod profile;

pub use bridge::VlessStreamBridge;
pub use inbound::{
    OwnedVlessInboundBindPlan, OwnedVlessInboundListenerConfig, VlessInboundListenerRequest,
};
pub use leaf::{OwnedVlessOutboundLeafConfig, VlessOutboundLeaf};
pub use outbound::OwnedVlessOutboundTransportPlan;
pub use profile::{
    OwnedVlessQuicBindProfile, OwnedVlessQuicClientProfile, OwnedVlessRealityClientProfile,
};
