//! Managed UDP flow request facade.
//!
//! The root stays as a facade so datagram flow inputs, stream flow inputs, and
//! shared request envelopes do not regrow into one implementation bucket.

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
mod datagram;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod envelope;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
mod stream;

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) use datagram::ManagedDatagramFlow;
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use envelope::{ManagedExistingFlowForward, ManagedUdpFlowKind, ManagedUdpFlowRequest};
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) use stream::{ManagedRelayStreamFlow, ManagedStreamPacketFlow};
