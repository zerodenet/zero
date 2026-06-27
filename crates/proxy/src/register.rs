//! Compiled protocol registration boundary.

use std::sync::Arc;

use crate::adapters::DirectAdapter;
#[cfg(feature = "http_connect")]
use crate::adapters::HttpConnectAdapter;
#[cfg(feature = "hysteria2")]
use crate::adapters::Hysteria2Adapter;
#[cfg(feature = "mieru")]
use crate::adapters::MieruAdapter;
#[cfg(feature = "mixed")]
use crate::adapters::MixedAdapter;
#[cfg(feature = "shadowsocks")]
use crate::adapters::ShadowsocksAdapter;
#[cfg(feature = "socks5")]
use crate::adapters::Socks5Adapter;
#[cfg(feature = "trojan")]
use crate::adapters::TrojanAdapter;
#[cfg(feature = "vless")]
use crate::adapters::VlessAdapter;
#[cfg(feature = "vmess")]
use crate::adapters::VmessAdapter;
use crate::protocol_registry::ProtocolRegistry;
use crate::runtime::udp_flow::managed::ManagedUdpHandlers;
use crate::runtime::udp_flow::registered::{RegisteredUdpHandlers, UpstreamUdpHandlers};

pub(crate) fn protocol_registry() -> ProtocolRegistry {
    let mut registry = ProtocolRegistry::default();

    #[cfg(feature = "socks5")]
    registry.register(Arc::new(Socks5Adapter));
    #[cfg(feature = "http_connect")]
    registry.register(Arc::new(HttpConnectAdapter));
    #[cfg(feature = "vless")]
    registry.register(Arc::new(VlessAdapter::default()));
    #[cfg(feature = "hysteria2")]
    registry.register(Arc::new(Hysteria2Adapter));
    #[cfg(feature = "shadowsocks")]
    registry.register(Arc::new(ShadowsocksAdapter));
    #[cfg(feature = "trojan")]
    registry.register(Arc::new(TrojanAdapter));
    #[cfg(feature = "vmess")]
    registry.register(Arc::new(VmessAdapter::default()));
    #[cfg(feature = "mieru")]
    registry.register(Arc::new(MieruAdapter));
    #[cfg(feature = "mixed")]
    registry.register(Arc::new(MixedAdapter));
    registry.register(Arc::new(DirectAdapter));

    registry
}

pub(crate) fn registered_udp_handlers() -> RegisteredUdpHandlers {
    RegisteredUdpHandlers {
        managed: ManagedUdpHandlers {
            datagram: vec![
                #[cfg(feature = "shadowsocks")]
                crate::adapters::shadowsocks_udp_datagram_handler(),
                #[cfg(feature = "hysteria2")]
                crate::adapters::hysteria2_udp_datagram_handler(),
            ],
            stream: vec![
                #[cfg(feature = "trojan")]
                crate::adapters::trojan_udp_stream_handler(),
                #[cfg(feature = "mieru")]
                crate::adapters::mieru_udp_stream_handler(),
            ],
        },
        upstream: UpstreamUdpHandlers {
            upstream: vec![
                #[cfg(feature = "socks5")]
                crate::adapters::socks5_upstream_association_handler(),
            ],
        },
    }
}
