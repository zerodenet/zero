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
#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::ManagedUdpHandlers;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::registered::RegisteredUdpHandlers;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[cfg(feature = "socks5")]
use crate::runtime::udp_flow::registered::UpstreamUdpHandlers;

fn compiled_protocol_registry() -> ProtocolRegistry {
    let mut registry = ProtocolRegistry::default();
    #[cfg(feature = "socks5")]
    registry.register_upstream_capability(Arc::new(Socks5Adapter));
    #[cfg(feature = "http_connect")]
    registry.register_core_capability(Arc::new(HttpConnectAdapter));
    #[cfg(feature = "vless")]
    registry.register_managed_capability(Arc::new(VlessAdapter::default()));
    #[cfg(feature = "hysteria2")]
    registry.register_managed_capability(Arc::new(Hysteria2Adapter));
    #[cfg(feature = "shadowsocks")]
    registry.register_managed_capability(Arc::new(ShadowsocksAdapter));
    #[cfg(feature = "trojan")]
    registry.register_managed_capability(Arc::new(TrojanAdapter::default()));
    #[cfg(feature = "vmess")]
    registry.register_managed_capability(Arc::new(VmessAdapter::default()));
    #[cfg(feature = "mieru")]
    registry.register_managed_capability(Arc::new(MieruAdapter));
    #[cfg(feature = "mixed")]
    registry.register_core_capability(Arc::new(MixedAdapter));
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    registry.register_capability(Arc::new(DirectAdapter));
    #[cfg(not(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    )))]
    registry.register_core_capability(Arc::new(DirectAdapter));
    registry
}

pub(crate) fn protocol_registry() -> ProtocolRegistry {
    compiled_protocol_registry()
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
pub(crate) fn registered_udp_handlers(registry: &ProtocolRegistry) -> RegisteredUdpHandlers {
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    let (stream_packet, relay) = registry
        .managed_udp_handler_providers()
        .filter_map(|capability| capability.managed_stream_udp_handlers())
        .map(|handlers| (handlers.stream_packet, handlers.relay))
        .unzip();

    RegisteredUdpHandlers {
        #[cfg(any(
            feature = "vless",
            feature = "hysteria2",
            feature = "shadowsocks",
            feature = "trojan",
            feature = "vmess",
            feature = "mieru"
        ))]
        managed: ManagedUdpHandlers {
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            datagram: registry
                .managed_udp_handler_providers()
                .filter_map(|capability| capability.managed_datagram_udp_handler())
                .collect(),
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            stream_packet,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            relay,
        },
        #[cfg(feature = "socks5")]
        upstream: UpstreamUdpHandlers {
            upstream: registry
                .upstream_udp_handler_providers()
                .map(|provider| provider.upstream_association_handler())
                .collect(),
        },
    }
}
