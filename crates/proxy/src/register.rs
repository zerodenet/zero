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
use crate::protocol_registry::{ProtocolRegistry, RegisteredProtocolCapability};
use crate::runtime::udp_flow::managed::ManagedUdpHandlers;
use crate::runtime::udp_flow::registered::{RegisteredUdpHandlers, UpstreamUdpHandlers};

fn capability<T>(adapter: T) -> Arc<dyn RegisteredProtocolCapability>
where
    T: RegisteredProtocolCapability + 'static,
{
    Arc::new(adapter)
}

fn compiled_protocol_adapters() -> Vec<Arc<dyn RegisteredProtocolCapability>> {
    vec![
        #[cfg(feature = "socks5")]
        capability(Socks5Adapter),
        #[cfg(feature = "http_connect")]
        capability(HttpConnectAdapter),
        #[cfg(feature = "vless")]
        capability(VlessAdapter::default()),
        #[cfg(feature = "hysteria2")]
        capability(Hysteria2Adapter),
        #[cfg(feature = "shadowsocks")]
        capability(ShadowsocksAdapter),
        #[cfg(feature = "trojan")]
        capability(TrojanAdapter::default()),
        #[cfg(feature = "vmess")]
        capability(VmessAdapter::default()),
        #[cfg(feature = "mieru")]
        capability(MieruAdapter),
        #[cfg(feature = "mixed")]
        capability(MixedAdapter),
        capability(DirectAdapter),
    ]
}

pub(crate) fn protocol_registry() -> ProtocolRegistry {
    let mut registry = ProtocolRegistry::default();
    for adapter in compiled_protocol_adapters() {
        registry.register_capability(adapter);
    }

    registry
}

pub(crate) fn registered_udp_handlers() -> RegisteredUdpHandlers {
    let adapters = compiled_protocol_adapters();

    RegisteredUdpHandlers {
        managed: ManagedUdpHandlers {
            datagram: adapters
                .iter()
                .filter_map(|adapter| adapter.managed_datagram_udp_handler())
                .collect(),
            stream: adapters
                .iter()
                .filter_map(|adapter| adapter.managed_stream_udp_handler())
                .collect(),
        },
        upstream: UpstreamUdpHandlers {
            upstream: adapters
                .into_iter()
                .filter_map(|adapter| adapter.upstream_association_handler())
                .collect(),
        },
    }
}
