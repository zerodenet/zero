//! Managed UDP flow request facade.
//!
//! The root stays as a facade so datagram flow inputs, stream flow inputs, and
//! shared request envelopes do not regrow into one implementation bucket.

#[cfg(feature = "managed-datagram-runtime")]
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
#[cfg(feature = "managed-stream-runtime")]
mod stream;

#[cfg(feature = "managed-datagram-runtime")]
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
#[cfg(feature = "managed-stream-runtime")]
pub(crate) use stream::{ManagedRelayStreamFlow, ManagedStreamPacketFlow};
