//! Kernel inbound protocol trait and unified session pipeline.
//!
//! The `InboundProtocol` trait is the boundary between protocol-specific
//! client response/relay handling and the kernel's protocol-agnostic pipeline.
//! Carrier accept, protocol authentication, and route/session construction
//! happen before `serve_inbound()` handoff. The kernel owns connection
//! counting, rate limiting, routing, metering, and session
//! lifecycle; protocol handlers never touch those directly.

mod contract;
mod lifecycle;

pub(crate) use contract::{
    ClientResponseInboundProtocol, InboundProtocol, NoClientResponseInboundProtocol,
    NoClientResponseStreamProtocol,
};
pub(crate) use lifecycle::{
    apply_kernel_rate_limits, serve_inbound, serve_inbound_with_client_response,
};
