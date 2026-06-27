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
use crate::protocol_adapter::ProtocolRegistry;
use crate::protocol_runtime::udp::{CachedUdpHandlers, ManagedUdpHandlers, ProtocolUdpHandlers};

pub(crate) fn protocol_registry() -> ProtocolRegistry {
    let mut registry = ProtocolRegistry::default();

    #[cfg(feature = "socks5")]
    registry.register(Arc::new(Socks5Adapter));
    #[cfg(feature = "http_connect")]
    registry.register(Arc::new(HttpConnectAdapter));
    #[cfg(feature = "vless")]
    registry.register(Arc::new(VlessAdapter));
    #[cfg(feature = "hysteria2")]
    registry.register(Arc::new(Hysteria2Adapter));
    #[cfg(feature = "shadowsocks")]
    registry.register(Arc::new(ShadowsocksAdapter));
    #[cfg(feature = "trojan")]
    registry.register(Arc::new(TrojanAdapter));
    #[cfg(feature = "vmess")]
    registry.register(Arc::new(VmessAdapter));
    #[cfg(feature = "mieru")]
    registry.register(Arc::new(MieruAdapter));
    #[cfg(feature = "mixed")]
    registry.register(Arc::new(MixedAdapter));
    registry.register(Arc::new(DirectAdapter));

    registry
}

pub(crate) fn protocol_udp_handlers() -> ProtocolUdpHandlers {
    ProtocolUdpHandlers {
        cached: CachedUdpHandlers {
            cached: vec![
                crate::protocol_runtime::udp::vless_cached_handler(),
                #[cfg(feature = "vmess")]
                crate::protocol_runtime::udp::vmess_cached_handler(),
            ],
        },
        managed: ManagedUdpHandlers {
            datagram: vec![
                #[cfg(feature = "shadowsocks")]
                crate::protocol_runtime::udp::shadowsocks_datagram_handler(),
                #[cfg(feature = "hysteria2")]
                crate::protocol_runtime::udp::hysteria2_datagram_handler(),
            ],
            stream: vec![
                #[cfg(feature = "trojan")]
                crate::protocol_runtime::udp::trojan_stream_handler(),
                #[cfg(feature = "mieru")]
                crate::protocol_runtime::udp::mieru_stream_handler(),
            ],
        },
    }
}
