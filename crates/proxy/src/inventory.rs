use crate::protocol_registry::ProtocolRegistry;

mod inbound;
mod metadata;
mod protocols;
mod runtime;
mod tcp;
#[cfg(test)]
mod tests;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod udp;

pub(crate) use runtime::{ClaimedInventoryLeaf, ClaimedRelayChain};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use tcp::dispatch_prepared_tcp_relay_carrier;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use tcp::PreparedTcpRelayChain;
pub(crate) use tcp::{dispatch_tcp_outbound, PreparedTcpOutbound};
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use udp::start_udp_resolved_outbound;

#[derive(Debug, Clone)]
pub struct ProtocolInventory {
    registry: ProtocolRegistry,
}

impl Default for ProtocolInventory {
    fn default() -> Self {
        Self {
            registry: crate::register::protocol_registry(),
        }
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl ProtocolInventory {
    pub(crate) fn registered_udp_handlers(
        &self,
    ) -> crate::runtime::udp_flow::registered::RegisteredUdpHandlers {
        crate::register::registered_udp_handlers(&self.registry)
    }
}
