//! Neutral execution machinery for resumable managed UDP flows.
//!
//! Concrete resume values remain opaque and are supplied by registered
//! protocol handlers.

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
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
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
mod datagram;
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
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
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
mod stream;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
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
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) use model::ManagedDatagramFlowHandler;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
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
