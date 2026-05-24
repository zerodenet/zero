//! Protocol adapter registry — eliminates per-protocol match arms in the proxy.
//!
//! Each protocol provides a `ProtocolAdapter` that knows its name, feature gate,
//! and how to validate its configuration.  The `ProtocolRegistry` collects
//! adapters at startup and replaces the hard-coded match statements in
//! `ProtocolInventory`.

use std::fmt;
use std::sync::Arc;

use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::EngineError;

/// A protocol adapter registered in the proxy.
///
/// Implementations are behind `#[cfg(feature = "...")]` gates so only
/// compiled-in protocols appear in the registry.
pub trait ProtocolAdapter: Send + Sync + fmt::Debug {
    /// Protocol name used in config `"type"` field and exported status.
    fn name(&self) -> &'static str;

    /// Cargo feature that gates this protocol (e.g. `"inbound-socks5"`).
    fn feature_name(&self) -> &'static str;

    /// Whether this adapter can handle the given inbound config.
    fn supports_inbound(&self, config: &InboundProtocolConfig) -> bool;

    /// Whether this adapter can handle the given outbound config.
    fn supports_outbound(&self, config: &OutboundProtocolConfig) -> bool;

    /// Whether this adapter provides an inbound listener.
    fn has_inbound(&self) -> bool;

    /// Whether this adapter provides an outbound connector.
    fn has_outbound(&self) -> bool;
}

/// Registry of all compiled-in protocol adapters.
///
/// Constructed at proxy startup via `build_registry()`.  Replaces the manual
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
    pub fn register(&mut self, adapter: Arc<dyn ProtocolAdapter>) {
        self.adapters.push(adapter);
    }

    /// Names of all compiled-in inbound protocols.
    pub fn inbound_names(&self) -> Vec<&'static str> {
        let mut names = self
            .adapters
            .iter()
            .filter(|a| a.has_inbound())
            .map(|a| a.name())
            .collect::<Vec<_>>();
        if cfg!(feature = "inbound-mixed") {
            names.push("mixed");
        }
        names
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
        self.adapters.iter().any(|a| a.supports_inbound(config))
            || matches!(config, InboundProtocolConfig::Mixed { .. })
    }

    pub fn supports_outbound(&self, config: &OutboundProtocolConfig) -> bool {
        matches!(
            config,
            OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block
        ) || self.adapters.iter().any(|a| a.supports_outbound(config))
    }

    /// Human-readable label for an inbound protocol config.
    pub fn inbound_protocol_label(&self, config: &InboundProtocolConfig) -> &'static str {
        for adapter in &self.adapters {
            if adapter.supports_inbound(config) {
                return adapter.name();
            }
        }
        if matches!(config, InboundProtocolConfig::Mixed { .. }) {
            return "mixed";
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
        if matches!(config, InboundProtocolConfig::Mixed { .. }) {
            return "inbound-mixed";
        }
        "protocol-not-compiled"
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
            _ => "protocol-not-compiled",
        }
    }
}
