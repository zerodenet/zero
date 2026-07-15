//! Compiled protocol registration boundary.

use std::sync::Arc;

use crate::adapters::DirectAdapter;
#[cfg(feature = "http")]
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
    registry.register_upstream_capability(
        Arc::new(Socks5Adapter),
        Socks5Adapter::claim_outbound_leaf_impl,
    );
    #[cfg(feature = "http")]
    registry.register_core_capability(Arc::new(HttpConnectAdapter), None);
    #[cfg(feature = "vless")]
    {
        let adapter = Arc::new(VlessAdapter::default());
        registry.register_managed_capability(adapter, VlessAdapter::claim_outbound_leaf_impl);
    }
    #[cfg(feature = "hysteria2")]
    registry.register_managed_capability(
        Arc::new(Hysteria2Adapter),
        Hysteria2Adapter::claim_outbound_leaf_impl,
    );
    #[cfg(feature = "shadowsocks")]
    registry.register_managed_capability(
        Arc::new(ShadowsocksAdapter),
        ShadowsocksAdapter::claim_outbound_leaf_impl,
    );
    #[cfg(feature = "trojan")]
    {
        let adapter = Arc::new(TrojanAdapter);
        registry.register_managed_capability(adapter, TrojanAdapter::claim_outbound_leaf_impl);
    }
    #[cfg(feature = "vmess")]
    {
        let adapter = Arc::new(VmessAdapter::default());
        registry.register_managed_capability(adapter, VmessAdapter::claim_outbound_leaf_impl);
    }
    #[cfg(feature = "mieru")]
    registry.register_managed_capability(
        Arc::new(MieruAdapter),
        MieruAdapter::claim_outbound_leaf_impl,
    );
    #[cfg(feature = "mixed")]
    registry.register_core_capability(Arc::new(MixedAdapter), None);
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    registry.register_capability(
        Arc::new(DirectAdapter),
        DirectAdapter::claim_outbound_leaf_impl,
    );
    #[cfg(not(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    )))]
    registry.register_core_capability(
        Arc::new(DirectAdapter),
        Some(DirectAdapter::claim_outbound_leaf_impl),
    );
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
