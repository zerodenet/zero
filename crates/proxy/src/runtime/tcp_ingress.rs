//! Post-accept TCP ingress contract and unified session pipeline.
//!
//! The `InboundProtocol` trait is the boundary between protocol-specific
//! client response/relay handling and the kernel's protocol-agnostic pipeline.
//! Carrier accept, protocol authentication, and route/session construction
//! happen before `serve_inbound()` handoff. The kernel owns connection
//! counting, rate limiting, routing, metering, and session
//! lifecycle; protocol handlers never touch those directly.

mod contract;
mod lifecycle;

#[cfg(any(
    feature = "vless",
    feature = "socks5",
    feature = "hysteria2",
    feature = "mieru"
))]
#[cfg(feature = "vless")]
pub(crate) use contract::ClientResponseInboundProtocol;
pub(crate) use contract::InboundProtocol;
#[cfg(any(feature = "vmess", feature = "trojan"))]
pub(crate) use contract::NoClientResponseInboundProtocol;
pub(crate) use contract::NoClientResponseStreamProtocol;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use lifecycle::apply_kernel_rate_limits;
pub(crate) use lifecycle::serve_inbound;
#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
pub(crate) use lifecycle::serve_inbound_with_client_response;
