use zero_config::{InboundProtocolConfig, OutboundProtocolConfig, RuntimeConfig};

use zero_engine::EngineError;

use crate::transport::DirectConnector;

#[cfg(feature = "inbound-http-connect")]
use zero_protocol_http_connect::HttpConnectInbound;
#[cfg(feature = "inbound-socks5")]
use zero_protocol_socks5::Socks5Inbound;
#[cfg(feature = "outbound-socks5")]
use zero_protocol_socks5::Socks5Outbound;
#[cfg(feature = "inbound-vless")]
use zero_protocol_vless::VlessInbound;
#[cfg(feature = "outbound-vless")]
use zero_protocol_vless::VlessOutbound;
#[cfg(feature = "inbound-hysteria2")]
use zero_protocol_hysteria2::Hysteria2Inbound;
#[cfg(feature = "outbound-hysteria2")]
use zero_protocol_hysteria2::Hysteria2Outbound;
#[cfg(feature = "inbound-shadowsocks")]
use zero_protocol_shadowsocks::ShadowsocksInbound;
#[cfg(feature = "outbound-shadowsocks")]
use zero_protocol_shadowsocks::ShadowsocksOutbound;
#[cfg(feature = "inbound-trojan")]
use zero_protocol_trojan::TrojanInbound;
#[cfg(feature = "outbound-trojan")]
use zero_protocol_trojan::TrojanOutbound;

#[derive(Debug, Default, Clone, Copy)]
pub struct ProtocolInventory {
    #[cfg(feature = "inbound-socks5")]
    pub socks5_inbound: Socks5Inbound,
    #[cfg(feature = "outbound-socks5")]
    pub socks5_outbound: Socks5Outbound,
    #[cfg(feature = "inbound-http-connect")]
    pub http_connect_inbound: HttpConnectInbound,
    #[cfg(feature = "inbound-vless")]
    pub vless_inbound: VlessInbound,
    #[cfg(feature = "outbound-vless")]
    pub vless_outbound: VlessOutbound,
    #[cfg(feature = "inbound-hysteria2")]
    pub hysteria2_inbound: Hysteria2Inbound,
    #[cfg(feature = "outbound-hysteria2")]
    pub hysteria2_outbound: Hysteria2Outbound,
    #[cfg(feature = "inbound-shadowsocks")]
    pub shadowsocks_inbound: ShadowsocksInbound,
    #[cfg(feature = "outbound-shadowsocks")]
    pub shadowsocks_outbound: ShadowsocksOutbound,
    #[cfg(feature = "inbound-trojan")]
    pub trojan_inbound: TrojanInbound,
    #[cfg(feature = "outbound-trojan")]
    pub trojan_outbound: TrojanOutbound,
    pub(crate) direct_outbound: DirectConnector,
}

impl ProtocolInventory {
    pub fn supported_inbounds(&self) -> Vec<&'static str> {
        let mut protocols = Vec::new();

        if cfg!(feature = "inbound-socks5") {
            protocols.push("socks5");
        }
        if cfg!(feature = "inbound-http-connect") {
            protocols.push("http-connect");
        }
        if cfg!(feature = "inbound-mixed") {
            protocols.push("mixed");
        }
        if cfg!(feature = "inbound-vless") {
            protocols.push("vless");
        }
        if cfg!(feature = "inbound-hysteria2") {
            protocols.push("hysteria2");
        }
        if cfg!(feature = "inbound-shadowsocks") {
            protocols.push("shadowsocks");
        }
        if cfg!(feature = "inbound-trojan") {
            protocols.push("trojan");
        }

        protocols
    }

    pub fn supported_outbounds(&self) -> Vec<&'static str> {
        let mut protocols = vec!["direct", "block"];

        if cfg!(feature = "outbound-socks5") {
            protocols.push("socks5");
        }
        if cfg!(feature = "outbound-vless") {
            protocols.push("vless");
        }
        if cfg!(feature = "outbound-hysteria2") {
            protocols.push("hysteria2");
        }
        if cfg!(feature = "outbound-shadowsocks") {
            protocols.push("shadowsocks");
        }
        if cfg!(feature = "outbound-trojan") {
            protocols.push("trojan");
        }

        protocols
    }

    pub fn validate_config(&self, config: &RuntimeConfig) -> Result<(), EngineError> {
        for inbound in &config.inbounds {
            if self.supports_inbound_protocol(&inbound.protocol) {
                continue;
            }

            return Err(EngineError::CompiledFeatureDisabled {
                kind: "inbound",
                tag: inbound.tag.clone(),
                protocol: inbound_protocol_name(&inbound.protocol),
                feature: inbound_protocol_feature(&inbound.protocol),
            });
        }

        for outbound in &config.outbounds {
            if self.supports_outbound_protocol(&outbound.protocol) {
                continue;
            }

            return Err(EngineError::CompiledFeatureDisabled {
                kind: "outbound",
                tag: outbound.tag.clone(),
                protocol: outbound_protocol_name(&outbound.protocol),
                feature: outbound_protocol_feature(&outbound.protocol),
            });
        }

        Ok(())
    }

    pub fn supports_inbound_protocol(&self, protocol: &InboundProtocolConfig) -> bool {
        match protocol {
            InboundProtocolConfig::Socks5 { .. } => cfg!(feature = "inbound-socks5"),
            InboundProtocolConfig::HttpConnect => cfg!(feature = "inbound-http-connect"),
            InboundProtocolConfig::Mixed { .. } => cfg!(feature = "inbound-mixed"),
            InboundProtocolConfig::Vless { .. } => cfg!(feature = "inbound-vless"),
            InboundProtocolConfig::Hysteria2 { .. } => cfg!(feature = "inbound-hysteria2"),
            InboundProtocolConfig::Shadowsocks { .. } => cfg!(feature = "inbound-shadowsocks"),
            InboundProtocolConfig::Trojan { .. } => cfg!(feature = "inbound-trojan"),
        }
    }

    pub fn supports_outbound_protocol(&self, protocol: &OutboundProtocolConfig) -> bool {
        match protocol {
            OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block => true,
            OutboundProtocolConfig::Socks5 { .. } => cfg!(feature = "outbound-socks5"),
            OutboundProtocolConfig::Vless { .. } => cfg!(feature = "outbound-vless"),
            OutboundProtocolConfig::Hysteria2 { .. } => cfg!(feature = "outbound-hysteria2"),
            OutboundProtocolConfig::Shadowsocks { .. } => cfg!(feature = "outbound-shadowsocks"),
            OutboundProtocolConfig::Trojan { .. } => cfg!(feature = "outbound-trojan"),
        }
    }
}

fn inbound_protocol_name(protocol: &InboundProtocolConfig) -> &'static str {
    match protocol {
        InboundProtocolConfig::Socks5 { .. } => "socks5",
        InboundProtocolConfig::HttpConnect => "http-connect",
        InboundProtocolConfig::Mixed { .. } => "mixed",
        InboundProtocolConfig::Vless { .. } => "vless",
        InboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
        InboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
        InboundProtocolConfig::Trojan { .. } => "trojan",
    }
}

fn inbound_protocol_feature(protocol: &InboundProtocolConfig) -> &'static str {
    match protocol {
        InboundProtocolConfig::Socks5 { .. } => "inbound-socks5",
        InboundProtocolConfig::HttpConnect => "inbound-http-connect",
        InboundProtocolConfig::Mixed { .. } => "inbound-mixed",
        InboundProtocolConfig::Vless { .. } => "inbound-vless",
        InboundProtocolConfig::Hysteria2 { .. } => "inbound-hysteria2",
        InboundProtocolConfig::Shadowsocks { .. } => "inbound-shadowsocks",
        InboundProtocolConfig::Trojan { .. } => "inbound-trojan",
    }
}

fn outbound_protocol_name(protocol: &OutboundProtocolConfig) -> &'static str {
    match protocol {
        OutboundProtocolConfig::Direct => "direct",
        OutboundProtocolConfig::Block => "block",
        OutboundProtocolConfig::Socks5 { .. } => "socks5",
        OutboundProtocolConfig::Vless { .. } => "vless",
        OutboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
        OutboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
        OutboundProtocolConfig::Trojan { .. } => "trojan",
    }
}

fn outbound_protocol_feature(protocol: &OutboundProtocolConfig) -> &'static str {
    match protocol {
        OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block => {
            unreachable!("core outbounds are always compiled")
        }
        OutboundProtocolConfig::Socks5 { .. } => "outbound-socks5",
        OutboundProtocolConfig::Vless { .. } => "outbound-vless",
        OutboundProtocolConfig::Hysteria2 { .. } => "outbound-hysteria2",
        OutboundProtocolConfig::Shadowsocks { .. } => "outbound-shadowsocks",
        OutboundProtocolConfig::Trojan { .. } => "outbound-trojan",
    }
}
