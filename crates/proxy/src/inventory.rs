use zero_config::{InboundProtocolConfig, OutboundProtocolConfig, RuntimeConfig};

use zero_engine::EngineError;

use crate::adapters::build_registry;
use crate::protocol_adapter::ProtocolRegistry;
use crate::transport::DirectConnector;

#[cfg(feature = "inbound-http-connect")]
use zero_protocol_http_connect::HttpConnectInbound;
#[cfg(feature = "inbound-hysteria2")]
use zero_protocol_hysteria2::Hysteria2Inbound;
#[cfg(feature = "outbound-hysteria2")]
use zero_protocol_hysteria2::Hysteria2Outbound;
#[cfg(feature = "inbound-shadowsocks")]
use zero_protocol_shadowsocks::ShadowsocksInbound;
#[cfg(feature = "outbound-shadowsocks")]
use zero_protocol_shadowsocks::ShadowsocksOutbound;
#[cfg(feature = "inbound-socks5")]
use zero_protocol_socks5::Socks5Inbound;
#[cfg(feature = "outbound-socks5")]
use zero_protocol_socks5::Socks5Outbound;
#[cfg(feature = "inbound-trojan")]
use zero_protocol_trojan::TrojanInbound;
#[cfg(feature = "outbound-trojan")]
use zero_protocol_trojan::TrojanOutbound;
#[cfg(feature = "inbound-vless")]
use zero_protocol_vless::VlessInbound;
#[cfg(feature = "outbound-vless")]
use zero_protocol_vless::VlessOutbound;

#[derive(Debug, Clone)]
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
    registry: ProtocolRegistry,
}

impl Default for ProtocolInventory {
    fn default() -> Self {
        Self {
            #[cfg(feature = "inbound-socks5")]
            socks5_inbound: Socks5Inbound,
            #[cfg(feature = "outbound-socks5")]
            socks5_outbound: Socks5Outbound,
            #[cfg(feature = "inbound-http-connect")]
            http_connect_inbound: HttpConnectInbound,
            #[cfg(feature = "inbound-vless")]
            vless_inbound: VlessInbound,
            #[cfg(feature = "outbound-vless")]
            vless_outbound: VlessOutbound,
            #[cfg(feature = "inbound-hysteria2")]
            hysteria2_inbound: Hysteria2Inbound,
            #[cfg(feature = "outbound-hysteria2")]
            hysteria2_outbound: Hysteria2Outbound,
            #[cfg(feature = "inbound-shadowsocks")]
            shadowsocks_inbound: ShadowsocksInbound,
            #[cfg(feature = "outbound-shadowsocks")]
            shadowsocks_outbound: ShadowsocksOutbound,
            #[cfg(feature = "inbound-trojan")]
            trojan_inbound: TrojanInbound,
            #[cfg(feature = "outbound-trojan")]
            trojan_outbound: TrojanOutbound,
            direct_outbound: DirectConnector,
            registry: build_registry(),
        }
    }
}

impl ProtocolInventory {
    pub fn supported_inbounds(&self) -> Vec<&'static str> {
        self.registry.inbound_names()
    }

    pub fn supported_outbounds(&self) -> Vec<&'static str> {
        self.registry.outbound_names()
    }

    pub fn validate_config(&self, config: &RuntimeConfig) -> Result<(), EngineError> {
        self.registry.validate_inbounds(&config.inbounds)?;
        self.registry.validate_outbounds(&config.outbounds)?;
        Ok(())
    }

    pub fn supports_inbound_protocol(&self, protocol: &InboundProtocolConfig) -> bool {
        self.registry.supports_inbound(protocol)
    }

    pub fn supports_outbound_protocol(&self, protocol: &OutboundProtocolConfig) -> bool {
        self.registry.supports_outbound(protocol)
    }

    pub(crate) fn check_inbound_enabled(
        &self,
        protocol: &InboundProtocolConfig,
        tag: &str,
    ) -> Result<(), EngineError> {
        if self.registry.supports_inbound(protocol) {
            return Ok(());
        }
        let label = self.registry.inbound_protocol_label(protocol);
        let feature = self.registry.inbound_protocol_feature_name(protocol);
        Err(EngineError::CompiledFeatureDisabled {
            kind: "inbound",
            tag: tag.to_owned(),
            protocol: label,
            feature,
        })
    }
}
