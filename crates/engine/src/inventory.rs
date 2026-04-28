use zero_config::{InboundProtocolConfig, OutboundProtocolConfig, RuntimeConfig};

use crate::engine::EngineError;
use crate::outbound::{BlockOutbound, DirectOutbound};

#[cfg(feature = "inbound-http-connect")]
use zero_protocol_http_connect::HttpConnectInbound;
#[cfg(any(feature = "inbound-socks5", feature = "outbound-socks5"))]
use zero_protocol_socks5::{Socks5Inbound, Socks5Outbound};
#[cfg(feature = "inbound-vless")]
use zero_protocol_vless::VlessInbound;
#[cfg(feature = "outbound-vless")]
use zero_protocol_vless::VlessOutbound;

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
    pub direct_outbound: DirectOutbound,
    pub block_outbound: BlockOutbound,
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
        }
    }

    pub fn supports_outbound_protocol(&self, protocol: &OutboundProtocolConfig) -> bool {
        match protocol {
            OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block => true,
            OutboundProtocolConfig::Socks5 { .. } => cfg!(feature = "outbound-socks5"),
            OutboundProtocolConfig::Vless { .. } => cfg!(feature = "outbound-vless"),
        }
    }
}

fn inbound_protocol_name(protocol: &InboundProtocolConfig) -> &'static str {
    match protocol {
        InboundProtocolConfig::Socks5 { .. } => "socks5",
        InboundProtocolConfig::HttpConnect => "http-connect",
        InboundProtocolConfig::Mixed { .. } => "mixed",
        InboundProtocolConfig::Vless { .. } => "vless",
    }
}

fn inbound_protocol_feature(protocol: &InboundProtocolConfig) -> &'static str {
    match protocol {
        InboundProtocolConfig::Socks5 { .. } => "inbound-socks5",
        InboundProtocolConfig::HttpConnect => "inbound-http-connect",
        InboundProtocolConfig::Mixed { .. } => "inbound-mixed",
        InboundProtocolConfig::Vless { .. } => "inbound-vless",
    }
}

fn outbound_protocol_name(protocol: &OutboundProtocolConfig) -> &'static str {
    match protocol {
        OutboundProtocolConfig::Direct => "direct",
        OutboundProtocolConfig::Block => "block",
        OutboundProtocolConfig::Socks5 { .. } => "socks5",
        OutboundProtocolConfig::Vless { .. } => "vless",
    }
}

fn outbound_protocol_feature(protocol: &OutboundProtocolConfig) -> &'static str {
    match protocol {
        OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block => {
            unreachable!("core outbounds are always compiled")
        }
        OutboundProtocolConfig::Socks5 { .. } => "outbound-socks5",
        OutboundProtocolConfig::Vless { .. } => "outbound-vless",
    }
}
