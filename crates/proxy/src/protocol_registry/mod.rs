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
mod transport_leaf;

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
    ClaimedTcpOutboundLeaf, InboundListenerCapability, OutboundLeafClaim,
    ProtocolSupportCapability, TcpOutboundCapability,
};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use capability::{
    ClaimedUdpFlowLeaf, ClaimedUdpPacketPathLeaf, UdpFlowCapability, UdpPacketPathCapability,
};
pub(crate) use context::{OutboundAdapterContext, TcpRuntimeServices};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use context::{UdpAdapterContext, UdpAssociationCloseKind, UdpRuntimeServices};
#[cfg(feature = "transport_quic")]
pub(crate) use defaults::bind_transport_inbound;
pub(crate) use model::{BoundInbound, OutboundLeafRuntime};
#[cfg(test)]
pub(crate) use registry::fake_direct_leaf;
pub(crate) use registry::ClaimedOutboundLeaf;
pub(crate) use registry::ProtocolRegistry;
#[cfg(feature = "vless")]
pub(crate) use transport_leaf::claim_relay_two_stream_transport_udp_leaf;
#[cfg(any(feature = "vless", feature = "vmess", feature = "trojan"))]
pub(crate) use transport_leaf::{claim_transport_tcp_leaf, claim_transport_udp_leaf};
