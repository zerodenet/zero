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
mod runtime;

pub use inbound::VlessInboundListenerRequest;
pub use leaf::VlessOutboundLeaf;
pub use profile::{
    VlessQuicBindOptionsRef, VlessQuicClientOptionsRef, VlessRealityClientOptionsRef,
};
pub use runtime::VlessTransportRuntime;
