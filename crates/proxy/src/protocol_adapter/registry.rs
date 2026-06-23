use std::fmt;
use std::sync::Arc;

use crate::adapters::{
    DirectAdapter, HttpConnectAdapter, Hysteria2Adapter, MieruAdapter, MixedAdapter,
    ShadowsocksAdapter, Socks5Adapter, TrojanAdapter, VlessAdapter, VmessAdapter,
};
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::protocol_adapter::{BoundInbound, OutboundLeafRuntime, ProtocolAdapter};
use crate::protocol_capability::{protocol_capability, protocol_descriptor};
use crate::runtime::orchestration::TcpPathCategory;

/// Registry of all compiled-in protocol adapters.
///
/// Constructed at proxy startup via `build()`. Replaces the manual
/// match arms in `ProtocolInventory::supports_*` and `protocol_name` functions.
#[derive(Clone, Default)]
pub struct ProtocolRegistry {
    adapters: Vec<Arc<dyn ProtocolAdapter>>,
}

impl fmt::Debug for ProtocolRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProtocolRegistry")
            .field("adapter_count", &self.adapters.len())
            .finish()
    }
}

impl ProtocolRegistry {
    pub(crate) fn build() -> Self {
        let mut r = Self::default();

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
        #[cfg(feature = "mixed")]
        r.register(Arc::new(MixedAdapter));
        r.register(Arc::new(DirectAdapter));

        r
    }

    pub fn register(&mut self, adapter: Arc<dyn ProtocolAdapter>) {
        self.adapters.push(adapter);
    }

    /// Names of all compiled-in inbound protocols.
    pub fn inbound_names(&self) -> Vec<&'static str> {
        self.adapters
            .iter()
            .filter(|a| a.has_inbound())
            .map(|a| a.name())
            .collect::<Vec<_>>()
    }

    /// Names of all compiled-in outbound protocols.
    pub fn outbound_names(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = vec!["direct", "block"];
        names.extend(
            self.adapters
                .iter()
                .filter(|a| a.has_outbound())
                .map(|a| a.name()),
        );
        names
    }

    pub fn capabilities(&self) -> Vec<zero_api::ProtocolCapability> {
        let mut descriptors = self
            .adapters
            .iter()
            .map(|adapter| adapter.descriptor())
            .collect::<Vec<_>>();

        if !descriptors
            .iter()
            .any(|descriptor| descriptor.protocol == "block")
        {
            descriptors.push(protocol_descriptor("block", "core"));
        }

        let mut capabilities = descriptors
            .into_iter()
            .map(protocol_capability)
            .collect::<Vec<_>>();
        capabilities.sort_by(|a, b| a.protocol.cmp(&b.protocol));
        capabilities
    }

    /// Validate that every inbound in the config has a compiled-in adapter.
    pub fn validate_inbounds(
        &self,
        configs: &[zero_config::InboundConfig],
    ) -> Result<(), EngineError> {
        for inbound in configs {
            if !self.supports_inbound(&inbound.protocol) {
                let name = self.inbound_protocol_label(&inbound.protocol);
                return Err(EngineError::CompiledFeatureDisabled {
                    kind: "inbound",
                    tag: inbound.tag.clone(),
                    protocol: name,
                    feature: self.inbound_protocol_feature_name(&inbound.protocol),
                });
            }
        }
        Ok(())
    }

    /// Validate that every outbound in the config has a compiled-in adapter.
    pub fn validate_outbounds(
        &self,
        configs: &[zero_config::OutboundConfig],
    ) -> Result<(), EngineError> {
        for outbound in configs {
            if !self.supports_outbound(&outbound.protocol) {
                let name = self.outbound_protocol_label(&outbound.protocol);
                return Err(EngineError::CompiledFeatureDisabled {
                    kind: "outbound",
                    tag: outbound.tag.clone(),
                    protocol: name,
                    feature: self.outbound_protocol_feature_name(&outbound.protocol),
                });
            }
        }
        Ok(())
    }

    pub fn supports_inbound(&self, config: &InboundProtocolConfig) -> bool {
        self.adapters
            .iter()
            .any(|adapter| adapter.supports_inbound(config))
    }

    pub fn supports_outbound(&self, config: &OutboundProtocolConfig) -> bool {
        matches!(
            config,
            OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block
        ) || self
            .adapters
            .iter()
            .any(|adapter| adapter.supports_outbound(config))
    }

    /// Human-readable label for an inbound protocol config.
    pub fn inbound_protocol_label(&self, config: &InboundProtocolConfig) -> &'static str {
        for adapter in &self.adapters {
            if adapter.supports_inbound(config) {
                return adapter.name();
            }
        }
        "unknown"
    }

    /// Cargo feature name needed to compile this inbound protocol.
    pub fn inbound_protocol_feature_name(&self, config: &InboundProtocolConfig) -> &'static str {
        for adapter in &self.adapters {
            if adapter.supports_inbound(config) {
                return adapter.feature_name();
            }
        }
        "protocol_not_compiled"
    }

    /// Human-readable label for an outbound protocol config.
    pub fn outbound_protocol_label(&self, config: &OutboundProtocolConfig) -> &'static str {
        for adapter in &self.adapters {
            if adapter.supports_outbound(config) {
                return adapter.name();
            }
        }
        match config {
            OutboundProtocolConfig::Direct => "direct",
            OutboundProtocolConfig::Block => "block",
            _ => "unknown",
        }
    }

    /// Cargo feature name needed to compile this outbound protocol.
    pub fn outbound_protocol_feature_name(&self, config: &OutboundProtocolConfig) -> &'static str {
        for adapter in &self.adapters {
            if adapter.supports_outbound(config) {
                return adapter.feature_name();
            }
        }
        match config {
            OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block => "core",
            _ => "protocol_not_compiled",
        }
    }

    /// Find the adapter that handles this inbound config, if any.
    ///
    /// Single dispatch point: the runtime resolves an inbound config to its
    /// adapter here instead of matching on the protocol enum.
    pub fn find_inbound(
        &self,
        config: &InboundProtocolConfig,
    ) -> Result<Arc<dyn ProtocolAdapter>, EngineError> {
        for adapter in &self.adapters {
            if adapter.supports_inbound(config) {
                return Ok(adapter.clone());
            }
        }
        let name = self.inbound_protocol_label(config);
        Err(EngineError::CompiledFeatureDisabled {
            kind: "inbound",
            tag: String::new(),
            protocol: name,
            feature: self.inbound_protocol_feature_name(config),
        })
    }

    /// Find the adapter that owns this resolved outbound leaf, if any.
    ///
    /// Single dispatch point: the TCP/UDP runtime resolves a
    /// [`ResolvedLeafOutbound`] to its adapter here instead of matching on
    /// the protocol enum. Each adapter claims exactly its own variant via
    /// [`ProtocolAdapter::claims_outbound_leaf`].
    pub fn find_outbound_leaf(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn ProtocolAdapter>, EngineError> {
        for adapter in &self.adapters {
            if adapter.claims_outbound_leaf(leaf) {
                return Ok(adapter.clone());
            }
        }
        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no compiled adapter handles this outbound leaf",
        )))
    }

    /// Return neutral runtime facts for a resolved outbound leaf.
    ///
    /// Kernel-level `block` is handled here because no adapter owns it.
    /// Direct and proxy protocols are delegated to the adapter that claims the
    /// leaf, so runtime code does not match protocol variants.
    pub(crate) fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<OutboundLeafRuntime<'a>, EngineError> {
        if let ResolvedLeafOutbound::Block { tag } = leaf {
            return Ok(OutboundLeafRuntime {
                tcp_path: TcpPathCategory::Block,
                health_tag: None,
                endpoint: None,
                kernel_tag: *tag,
            });
        }

        for adapter in &self.adapters {
            if !adapter.claims_outbound_leaf(leaf) {
                continue;
            }
            if let Some(runtime) = adapter.outbound_leaf_runtime(leaf) {
                return Ok(runtime);
            }
            break;
        }

        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no compiled adapter describes this outbound leaf",
        )))
    }

    /// Bind an inbound listener via its registered adapter.
    ///
    /// Single dispatch point: the runtime resolves an inbound config to its
    /// adapter and binds the socket here, instead of matching on the protocol
    /// enum. Port conflicts surface before the accept loop spawns.
    pub async fn bind_inbound(
        &self,
        inbound: &zero_config::InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        for adapter in &self.adapters {
            if adapter.supports_inbound(&inbound.protocol) {
                return adapter.bind_inbound(inbound, source_dir).await;
            }
        }
        let name = self.inbound_protocol_label(&inbound.protocol);
        Err(EngineError::CompiledFeatureDisabled {
            kind: "inbound",
            tag: inbound.tag.clone(),
            protocol: name,
            feature: self.inbound_protocol_feature_name(&inbound.protocol),
        })
    }
}

#[cfg(test)]
mod tests;
