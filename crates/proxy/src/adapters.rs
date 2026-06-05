//! Concrete `ProtocolAdapter` implementations for each compiled-in protocol.

use std::sync::Arc;

use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::protocol_capability::protocol_descriptor;

use super::protocol_adapter::ProtocolAdapter;

#[cfg(feature = "socks5")]
#[derive(Debug)]
pub(crate) struct Socks5Adapter;

#[cfg(feature = "socks5")]
impl ProtocolAdapter for Socks5Adapter {
    fn name(&self) -> &'static str {
        "socks5"
    }

    fn feature_name(&self) -> &'static str {
        "socks5"
    }

    fn has_inbound(&self) -> bool {
        true
    }

    fn has_outbound(&self) -> bool {
        true
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Socks5 { .. })
    }

    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Socks5 { .. })
    }
}

#[cfg(feature = "socks5")]
impl ProtocolMetadata for Socks5Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        socks5::Socks5Protocol.descriptor()
    }
}

#[cfg(feature = "http_connect")]
#[derive(Debug)]
pub(crate) struct HttpConnectAdapter;

#[cfg(feature = "http_connect")]
impl ProtocolAdapter for HttpConnectAdapter {
    fn name(&self) -> &'static str {
        "http_connect"
    }

    fn feature_name(&self) -> &'static str {
        "http_connect"
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::HttpConnect)
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

#[cfg(feature = "http_connect")]
impl ProtocolMetadata for HttpConnectAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        http_connect::HttpConnectProtocol.descriptor()
    }
}

#[cfg(feature = "vless")]
#[derive(Debug)]
pub(crate) struct VlessAdapter;

#[cfg(feature = "vless")]
impl ProtocolAdapter for VlessAdapter {
    fn name(&self) -> &'static str {
        "vless"
    }
    fn feature_name(&self) -> &'static str {
        "vless"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Vless { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Vless { .. })
    }
}

#[cfg(feature = "vless")]
impl ProtocolMetadata for VlessAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        vless::VlessProtocol.descriptor()
    }
}

#[cfg(feature = "hysteria2")]
#[derive(Debug)]
pub(crate) struct Hysteria2Adapter;

#[cfg(feature = "hysteria2")]
impl ProtocolAdapter for Hysteria2Adapter {
    fn name(&self) -> &'static str {
        "hysteria2"
    }
    fn feature_name(&self) -> &'static str {
        "hysteria2"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Hysteria2 { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Hysteria2 { .. })
    }
}

#[cfg(feature = "hysteria2")]
impl ProtocolMetadata for Hysteria2Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        hysteria2::Hysteria2Protocol.descriptor()
    }
}

#[cfg(feature = "shadowsocks")]
#[derive(Debug)]
pub(crate) struct ShadowsocksAdapter;

#[cfg(feature = "shadowsocks")]
impl ProtocolAdapter for ShadowsocksAdapter {
    fn name(&self) -> &'static str {
        "shadowsocks"
    }
    fn feature_name(&self) -> &'static str {
        "shadowsocks"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Shadowsocks { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Shadowsocks { .. })
    }
}

#[cfg(feature = "shadowsocks")]
impl ProtocolMetadata for ShadowsocksAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        shadowsocks::ShadowsocksProtocol.descriptor()
    }
}

#[cfg(feature = "trojan")]
#[derive(Debug)]
pub(crate) struct TrojanAdapter;

#[cfg(feature = "trojan")]
impl ProtocolAdapter for TrojanAdapter {
    fn name(&self) -> &'static str {
        "trojan"
    }
    fn feature_name(&self) -> &'static str {
        "trojan"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Trojan { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Trojan { .. })
    }
}

#[cfg(feature = "trojan")]
impl ProtocolMetadata for TrojanAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        trojan::TrojanProtocol.descriptor()
    }
}

#[cfg(feature = "vmess")]
#[derive(Debug)]
pub(crate) struct VmessAdapter;

#[cfg(feature = "vmess")]
impl ProtocolAdapter for VmessAdapter {
    fn name(&self) -> &'static str {
        "vmess"
    }
    fn feature_name(&self) -> &'static str {
        "vmess"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Vmess { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Vmess { .. })
    }
}

#[cfg(feature = "vmess")]
impl ProtocolMetadata for VmessAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        vmess::VmessProtocol.descriptor()
    }
}

#[cfg(feature = "mieru")]
#[derive(Debug)]
pub(crate) struct MieruAdapter;

#[cfg(feature = "mieru")]
impl ProtocolAdapter for MieruAdapter {
    fn name(&self) -> &'static str {
        "mieru"
    }
    fn feature_name(&self) -> &'static str {
        "mieru"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Mieru { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Mieru { .. })
    }
}

#[cfg(feature = "mieru")]
impl ProtocolMetadata for MieruAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        mieru::MieruProtocol.descriptor()
    }
}

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

impl ProtocolMetadata for DirectAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        protocol_descriptor("direct", "core")
    }
}

/// Build and return the protocol registry with all compiled-in adapters.
pub(crate) fn build_registry() -> super::protocol_adapter::ProtocolRegistry {
    let mut r = super::protocol_adapter::ProtocolRegistry::default();

    #[cfg(feature = "socks5")]
    r.register(Arc::new(Socks5Adapter));
    #[cfg(feature = "http_connect")]
    r.register(Arc::new(HttpConnectAdapter));
    #[cfg(feature = "vless")]
    r.register(Arc::new(VlessAdapter));
    #[cfg(feature = "hysteria2")]
    r.register(Arc::new(Hysteria2Adapter));
    #[cfg(feature = "shadowsocks")]
    r.register(Arc::new(ShadowsocksAdapter));
    #[cfg(feature = "trojan")]
    r.register(Arc::new(TrojanAdapter));
    #[cfg(feature = "vmess")]
    r.register(Arc::new(VmessAdapter));
    #[cfg(feature = "mieru")]
    r.register(Arc::new(MieruAdapter));
    // Always available.
    r.register(Arc::new(DirectAdapter));

    r
}
