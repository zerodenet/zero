//! Concrete `ProtocolAdapter` implementations for each compiled-in protocol.

use std::sync::Arc;

use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};

use super::protocol_adapter::ProtocolAdapter;

macro_rules! protocol_adapter {
    (
        $struct_name:ident,
        proto: $proto_name:literal,
        feature: $feature:literal,
        inbound: $inbound_pat:pat,
        outbound: $outbound_pat:pat
    ) => {
        #[derive(Debug)]
        pub(crate) struct $struct_name;

        impl ProtocolAdapter for $struct_name {
            fn name(&self) -> &'static str {
                $proto_name
            }
            fn feature_name(&self) -> &'static str {
                $feature
            }
            fn has_inbound(&self) -> bool {
                cfg!(feature = $feature)
            }
            fn has_outbound(&self) -> bool {
                cfg!(feature = $feature)
            }

            fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
                matches!(c, $inbound_pat)
            }
            fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
                matches!(c, $outbound_pat)
            }
        }
    };
}

#[cfg(feature = "inbound-socks5")]
protocol_adapter!(Socks5Adapter, proto: "socks5", feature: "inbound-socks5",
    inbound: InboundProtocolConfig::Socks5 { .. },
    outbound: OutboundProtocolConfig::Socks5 { .. });

#[cfg(feature = "inbound-http-connect")]
protocol_adapter!(HttpConnectAdapter, proto: "http-connect", feature: "inbound-http-connect",
    inbound: InboundProtocolConfig::HttpConnect,
    outbound: OutboundProtocolConfig::Direct);

#[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
protocol_adapter!(VlessAdapter, proto: "vless", feature: "inbound-vless",
    inbound: InboundProtocolConfig::Vless { .. },
    outbound: OutboundProtocolConfig::Vless { .. });

#[cfg(any(feature = "inbound-hysteria2", feature = "outbound-hysteria2"))]
protocol_adapter!(Hysteria2Adapter, proto: "hysteria2", feature: "inbound-hysteria2",
    inbound: InboundProtocolConfig::Hysteria2 { .. },
    outbound: OutboundProtocolConfig::Hysteria2 { .. });

#[cfg(any(feature = "inbound-shadowsocks", feature = "outbound-shadowsocks"))]
protocol_adapter!(ShadowsocksAdapter, proto: "shadowsocks", feature: "inbound-shadowsocks",
    inbound: InboundProtocolConfig::Shadowsocks { .. },
    outbound: OutboundProtocolConfig::Shadowsocks { .. });

#[cfg(any(feature = "inbound-trojan", feature = "outbound-trojan"))]
protocol_adapter!(TrojanAdapter, proto: "trojan", feature: "inbound-trojan",
    inbound: InboundProtocolConfig::Trojan { .. },
    outbound: OutboundProtocolConfig::Trojan { .. });

#[cfg(any(feature = "inbound-vmess", feature = "outbound-vmess"))]
protocol_adapter!(VmessAdapter, proto: "vmess", feature: "inbound-vmess",
    inbound: InboundProtocolConfig::Vmess { .. },
    outbound: OutboundProtocolConfig::Vmess { .. });

// Direct inbound is always available (no feature gate).
#[derive(Debug)]
pub(crate) struct DirectAdapter;

impl ProtocolAdapter for DirectAdapter {
    fn name(&self) -> &'static str {
        "direct"
    }
    fn feature_name(&self) -> &'static str {
        "core"
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Direct { .. })
    }
    fn supports_outbound(&self, _: &OutboundProtocolConfig) -> bool {
        false
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        false
    }
}

/// Build and return the protocol registry with all compiled-in adapters.
pub(crate) fn build_registry() -> super::protocol_adapter::ProtocolRegistry {
    let mut r = super::protocol_adapter::ProtocolRegistry::default();

    #[cfg(feature = "inbound-socks5")]
    r.register(Arc::new(Socks5Adapter));
    #[cfg(feature = "inbound-http-connect")]
    r.register(Arc::new(HttpConnectAdapter));
    #[cfg(any(feature = "inbound-vless", feature = "outbound-vless"))]
    r.register(Arc::new(VlessAdapter));
    #[cfg(any(feature = "inbound-hysteria2", feature = "outbound-hysteria2"))]
    r.register(Arc::new(Hysteria2Adapter));
    #[cfg(any(feature = "inbound-shadowsocks", feature = "outbound-shadowsocks"))]
    r.register(Arc::new(ShadowsocksAdapter));
    #[cfg(any(feature = "inbound-trojan", feature = "outbound-trojan"))]
    r.register(Arc::new(TrojanAdapter));
    #[cfg(any(feature = "inbound-vmess", feature = "outbound-vmess"))]
    r.register(Arc::new(VmessAdapter));
    // Always available.
    r.register(Arc::new(DirectAdapter));

    r
}
