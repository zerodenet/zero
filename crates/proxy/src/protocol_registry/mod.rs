//! Protocol registry - eliminates per-protocol match arms in the proxy.
//!
//! Each registered protocol contributes focused capability traits for support
//! metadata, inbound listeners, TCP outbound, UDP flows, and packet-path roles.
//! The `ProtocolRegistry` collects capability objects at startup and replaces
//! hard-coded match statements in `ProtocolInventory`.

mod capability;
mod context;
mod defaults;
mod model;
mod registry;

#[cfg(any(
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) use capability::ManagedUdpHandlerProvider;
#[cfg(feature = "socks5")]
pub(crate) use capability::UpstreamUdpHandlerProvider;
pub(crate) use capability::{
    InboundListenerCapability, ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability,
    UdpPacketPathCapability,
};
pub(crate) use context::{InboundAdapterContext, OutboundAdapterContext, UdpAdapterContext};
#[cfg(feature = "transport_quic")]
pub(crate) use defaults::bind_transport_inbound;
pub(crate) use defaults::unreachable_leaf;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use defaults::unreachable_udp_leaf;
pub(crate) use model::{BoundInbound, OutboundLeafRuntime};
pub(crate) use registry::direct_leaf_runtime;
#[cfg(test)]
pub(crate) use registry::fake_direct_leaf;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use registry::proxy_leaf_runtime;
pub(crate) use registry::ProtocolRegistry;
