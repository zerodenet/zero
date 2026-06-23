use zero_config::{InboundProtocolConfig, OutboundProtocolConfig, RuntimeConfig};

use zero_api::ProtocolCapability;
use zero_engine::EngineError;

use crate::protocol_adapter::{BoundInbound, OutboundLeafRuntime, ProtocolRegistry};
use crate::transport::DirectConnector;

#[cfg(feature = "http_connect")]
use http_connect::HttpConnectInbound;
#[cfg(feature = "shadowsocks")]
use shadowsocks::ShadowsocksOutbound;
#[cfg(feature = "socks5")]
use socks5::Socks5Inbound;
#[cfg(feature = "socks5")]
use socks5::Socks5Outbound;
#[cfg(feature = "trojan")]
use trojan::TrojanOutbound;
#[cfg(feature = "vless")]
use vless::VlessInbound;
#[cfg(feature = "vless")]
use vless::VlessOutbound;
#[cfg(feature = "vmess")]
use vmess::VmessOutbound;

#[derive(Debug, Clone)]
pub struct ProtocolInventory {
    registry: ProtocolRegistry,
}

impl Default for ProtocolInventory {
    fn default() -> Self {
        Self {
            registry: ProtocolRegistry::build(),
        }
    }
}

impl ProtocolInventory {
    pub(crate) fn direct_connector(&self) -> DirectConnector {
        DirectConnector
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn socks5_inbound_protocol(&self) -> Socks5Inbound {
        Socks5Inbound
    }

    #[cfg(feature = "socks5")]
    pub(crate) fn socks5_outbound_protocol(&self) -> Socks5Outbound {
        Socks5Outbound
    }

    #[cfg(feature = "http_connect")]
    pub(crate) fn http_connect_inbound_protocol(&self) -> HttpConnectInbound {
        HttpConnectInbound
    }

    #[cfg(feature = "vless")]
    pub(crate) fn vless_inbound_protocol(&self) -> VlessInbound {
        VlessInbound
    }

    #[cfg(feature = "vless")]
    pub(crate) fn vless_outbound_protocol(&self) -> VlessOutbound {
        VlessOutbound
    }

    #[cfg(feature = "shadowsocks")]
    pub(crate) fn shadowsocks_outbound_protocol(&self) -> ShadowsocksOutbound {
        ShadowsocksOutbound
    }

    #[cfg(feature = "trojan")]
    pub(crate) fn trojan_outbound_protocol(&self) -> TrojanOutbound {
        TrojanOutbound
    }

    #[cfg(feature = "vmess")]
    pub(crate) fn vmess_outbound_protocol(&self) -> VmessOutbound {
        VmessOutbound
    }

    pub fn supported_inbounds(&self) -> Vec<&'static str> {
        self.registry.inbound_names()
    }

    pub fn supported_outbounds(&self) -> Vec<&'static str> {
        self.registry.outbound_names()
    }

    pub fn protocol_capabilities(&self) -> Vec<ProtocolCapability> {
        self.registry.capabilities()
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
    pub(crate) async fn bind_inbound(
        &self,
        inbound: &zero_config::InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        self.registry.bind_inbound(inbound, source_dir).await
    }

    /// Resolve a [`ResolvedLeafOutbound`] to its registered outbound adapter.
    ///
    /// Single dispatch point for TCP/UDP outbound establishment — the runtime
    /// calls this instead of matching on the protocol enum.
    pub(crate) fn find_outbound_leaf(
        &self,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Result<std::sync::Arc<dyn crate::protocol_adapter::ProtocolAdapter>, EngineError> {
        self.registry.find_outbound_leaf(leaf)
    }

    /// Return the runtime-neutral facts for a resolved outbound leaf.
    ///
    /// The runtime asks the inventory for this instead of matching concrete
    /// protocol variants.
    pub(crate) fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &zero_engine::ResolvedLeafOutbound<'a>,
    ) -> Result<OutboundLeafRuntime<'a>, EngineError> {
        self.registry.outbound_leaf_runtime(leaf)
    }

    /// Resolve an [`InboundProtocolConfig`] to its registered inbound adapter.
    ///
    /// Single dispatch point for inbound spawn — the runtime calls this
    /// instead of matching on the protocol enum.
    pub(crate) fn find_inbound(
        &self,
        config: &InboundProtocolConfig,
    ) -> Result<std::sync::Arc<dyn crate::protocol_adapter::ProtocolAdapter>, EngineError> {
        self.registry.find_inbound(config)
    }
}
