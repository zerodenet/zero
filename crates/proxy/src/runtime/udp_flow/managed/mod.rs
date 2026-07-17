//! Neutral execution machinery for resumable managed UDP flows.
//!
//! Concrete resume values remain opaque and are supplied by registered
//! protocol handlers.

#[cfg(feature = "managed-stream-runtime")]
pub(crate) mod bridge;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru",
    feature = "hysteria2",
    feature = "shadowsocks"
))]
mod cache;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru",
    feature = "hysteria2",
    feature = "shadowsocks"
))]
mod connection;
#[cfg(feature = "managed-datagram-runtime")]
mod datagram;
#[cfg(feature = "managed-datagram-runtime")]
pub(crate) mod datagram_manager;
mod flow;
pub(crate) mod model;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) mod state;
#[cfg(feature = "managed-stream-runtime")]
mod stream;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) mod stream_manager;
#[cfg(test)]
mod tests;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use flow::ManagedExistingFlowForward;
pub(crate) use flow::ManagedUdpFlowResume;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use flow::{ManagedUdpFlowKind, ManagedUdpFlowRequest};
#[cfg(feature = "managed-datagram-runtime")]
pub(crate) use model::ManagedDatagramFlowHandler;
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use model::ManagedStreamHandlerPair;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use state::{ManagedUdpHandlers, ManagedUdpState};
