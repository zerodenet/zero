use std::fmt;
use std::sync::Arc;

#[cfg(any(
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::protocol_registry::ManagedUdpHandlerProvider;
#[cfg(test)]
use crate::protocol_registry::TcpOutboundCapability;
#[cfg(feature = "socks5")]
use crate::protocol_registry::UpstreamUdpHandlerProvider;
use crate::protocol_registry::{
    InboundListenerCapability, OutboundLeafClaimCapability, ProtocolSupportCapability,
};
#[cfg(all(
    test,
    any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    )
))]
use crate::protocol_registry::{UdpFlowCapability, UdpPacketPathCapability};

mod build;
mod inbound;
mod metadata;
mod outbound;
mod runtime;
mod support;
mod validation;

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use outbound::proxy_leaf_runtime;
pub(crate) use outbound::{direct_leaf_runtime, ClaimedOutboundLeaf};

/// Registry of all compiled-in protocol adapters.
///
/// Constructed at proxy startup via `build()`. Replaces the manual
/// match arms in `ProtocolInventory::supports_*` and `protocol_name` functions.
#[derive(Clone, Default)]
pub(crate) struct ProtocolRegistry {
    entries: Vec<RegisteredProtocolEntry>,
}

#[derive(Clone)]
struct RegisteredProtocolEntry {
    support: Arc<dyn ProtocolSupportCapability>,
    inbound: Arc<dyn InboundListenerCapability>,
    outbound: Arc<dyn OutboundLeafClaimCapability>,
    #[cfg(test)]
    tcp: Arc<dyn TcpOutboundCapability>,
    #[cfg(all(
        test,
        any(
            feature = "socks5",
            feature = "vless",
            feature = "hysteria2",
            feature = "shadowsocks",
            feature = "trojan",
            feature = "vmess",
            feature = "mieru"
        )
    ))]
    udp: Option<Arc<dyn UdpFlowCapability>>,
    #[cfg(any(
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    managed_udp_handlers: Option<Arc<dyn ManagedUdpHandlerProvider>>,
    #[cfg(feature = "socks5")]
    upstream_udp_handler: Option<Arc<dyn UpstreamUdpHandlerProvider>>,
    #[cfg(all(
        test,
        any(
            feature = "socks5",
            feature = "vless",
            feature = "hysteria2",
            feature = "shadowsocks",
            feature = "trojan",
            feature = "vmess",
            feature = "mieru"
        )
    ))]
    packet_path: Option<Arc<dyn UdpPacketPathCapability>>,
}

impl fmt::Debug for ProtocolRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtocolRegistry")
            .field("entry_count", &self.entries.len())
            .finish()
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::fake_direct_leaf;
