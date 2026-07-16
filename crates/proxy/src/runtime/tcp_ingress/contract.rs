//! TCP ingress contract facade.
//!
//! The root stays as a facade so flow accounting, protocol trait defaults, and
//! client-response wrappers do not regrow into one implementation bucket.

mod accounting;
#[cfg(any(
    feature = "vless",
    feature = "socks5",
    feature = "hysteria2",
    feature = "mieru"
))]
mod client_response;
mod no_response;
mod protocol;

#[cfg(any(
    feature = "vless",
    feature = "socks5",
    feature = "hysteria2",
    feature = "mieru"
))]
pub(crate) use client_response::ClientResponseInboundProtocol;
#[cfg(any(feature = "vmess", feature = "trojan"))]
pub(crate) use no_response::NoClientResponseInboundProtocol;
pub(crate) use no_response::NoClientResponseStreamProtocol;
pub(crate) use protocol::InboundProtocol;
