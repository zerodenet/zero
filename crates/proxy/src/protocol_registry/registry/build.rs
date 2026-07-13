use std::sync::Arc;

use super::ProtocolRegistry;
#[cfg(any(
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
use crate::protocol_registry::ManagedUdpHandlerProvider;
#[cfg(feature = "socks5")]
use crate::protocol_registry::UpstreamUdpHandlerProvider;
use crate::protocol_registry::{
    InboundListenerCapability, ProtocolSupportCapability, TcpOutboundCapability,
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
use crate::protocol_registry::{UdpFlowCapability, UdpPacketPathCapability};

impl ProtocolRegistry {
    #[cfg(any(
        not(any(
            feature = "socks5",
            feature = "vless",
            feature = "hysteria2",
            feature = "shadowsocks",
            feature = "trojan",
            feature = "vmess",
            feature = "mieru"
        )),
        feature = "http_connect",
        feature = "mixed"
    ))]
    pub(crate) fn register_core_capability<T>(&mut self, adapter: Arc<T>)
    where
        T: ProtocolSupportCapability + InboundListenerCapability + TcpOutboundCapability + 'static,
    {
        self.entries.push(super::RegisteredProtocolEntry {
            support: adapter.clone(),
            inbound: adapter.clone(),
            tcp: adapter,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            udp: None,
            #[cfg(any(
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            managed_udp_handlers: None,
            #[cfg(feature = "socks5")]
            upstream_udp_handler: None,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            packet_path: None,
        });
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
    pub(crate) fn register_capability<T>(&mut self, adapter: Arc<T>)
    where
        T: ProtocolSupportCapability
            + InboundListenerCapability
            + TcpOutboundCapability
            + UdpFlowCapability
            + UdpPacketPathCapability
            + 'static,
    {
        self.entries.push(super::RegisteredProtocolEntry {
            support: adapter.clone(),
            inbound: adapter.clone(),
            tcp: adapter.clone(),
            udp: Some(adapter.clone()),
            #[cfg(any(
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            managed_udp_handlers: None,
            #[cfg(feature = "socks5")]
            upstream_udp_handler: None,
            packet_path: Some(adapter),
        });
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn register_upstream_capability<T>(&mut self, adapter: Arc<T>)
    where
        T: ProtocolSupportCapability
            + InboundListenerCapability
            + TcpOutboundCapability
            + UdpFlowCapability
            + UdpPacketPathCapability
            + UpstreamUdpHandlerProvider
            + 'static,
    {
        self.entries.push(super::RegisteredProtocolEntry {
            support: adapter.clone(),
            inbound: adapter.clone(),
            tcp: adapter.clone(),
            udp: Some(adapter.clone()),
            #[cfg(any(
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            managed_udp_handlers: None,
            upstream_udp_handler: Some(adapter.clone()),
            packet_path: Some(adapter),
        });
    }

    #[cfg(any(
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) fn register_managed_capability<T>(&mut self, adapter: Arc<T>)
    where
        T: ProtocolSupportCapability
            + InboundListenerCapability
            + TcpOutboundCapability
            + UdpFlowCapability
            + UdpPacketPathCapability
            + ManagedUdpHandlerProvider
            + 'static,
    {
        self.entries.push(super::RegisteredProtocolEntry {
            support: adapter.clone(),
            inbound: adapter.clone(),
            tcp: adapter.clone(),
            udp: Some(adapter.clone()),
            managed_udp_handlers: Some(adapter.clone()),
            #[cfg(feature = "socks5")]
            upstream_udp_handler: None,
            packet_path: Some(adapter),
        });
    }

    #[cfg(any(
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) fn managed_udp_handler_providers(
        &self,
    ) -> impl Iterator<Item = &Arc<dyn ManagedUdpHandlerProvider>> {
        self.entries
            .iter()
            .filter_map(|entry| entry.managed_udp_handlers.as_ref())
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn upstream_udp_handler_providers(
        &self,
    ) -> impl Iterator<Item = &Arc<dyn UpstreamUdpHandlerProvider>> {
        self.entries
            .iter()
            .filter_map(|entry| entry.upstream_udp_handler.as_ref())
    }
}
