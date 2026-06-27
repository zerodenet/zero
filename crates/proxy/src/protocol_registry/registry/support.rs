use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};

use super::ProtocolRegistry;
use crate::protocol_registry::ProtocolSupportCapability;

impl ProtocolRegistry {
    pub(crate) fn supports_inbound(&self, config: &InboundProtocolConfig) -> bool {
        self.adapters
            .iter()
            .any(|adapter| ProtocolSupportCapability::supports_inbound(adapter.as_ref(), config))
    }

    pub(crate) fn supports_outbound(&self, config: &OutboundProtocolConfig) -> bool {
        matches!(
            config,
            OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block
        ) || self
            .adapters
            .iter()
            .any(|adapter| ProtocolSupportCapability::supports_outbound(adapter.as_ref(), config))
    }

    /// Human-readable label for an inbound protocol config.
    pub(crate) fn inbound_protocol_label(&self, config: &InboundProtocolConfig) -> &'static str {
        for adapter in &self.adapters {
            if ProtocolSupportCapability::supports_inbound(adapter.as_ref(), config) {
                return ProtocolSupportCapability::name(adapter.as_ref());
            }
        }
        "unknown"
    }

    /// Cargo feature name needed to compile this inbound protocol.
    pub(crate) fn inbound_protocol_feature_name(
        &self,
        config: &InboundProtocolConfig,
    ) -> &'static str {
        for adapter in &self.adapters {
            if ProtocolSupportCapability::supports_inbound(adapter.as_ref(), config) {
                return ProtocolSupportCapability::feature_name(adapter.as_ref());
            }
        }
        "protocol_not_compiled"
    }

    /// Human-readable label for an outbound protocol config.
    pub(crate) fn outbound_protocol_label(&self, config: &OutboundProtocolConfig) -> &'static str {
        for adapter in &self.adapters {
            if ProtocolSupportCapability::supports_outbound(adapter.as_ref(), config) {
                return ProtocolSupportCapability::name(adapter.as_ref());
            }
        }
        match config {
            OutboundProtocolConfig::Direct => "direct",
            OutboundProtocolConfig::Block => "block",
            _ => "unknown",
        }
    }

    /// Cargo feature name needed to compile this outbound protocol.
    pub(crate) fn outbound_protocol_feature_name(
        &self,
        config: &OutboundProtocolConfig,
    ) -> &'static str {
        for adapter in &self.adapters {
            if ProtocolSupportCapability::supports_outbound(adapter.as_ref(), config) {
                return ProtocolSupportCapability::feature_name(adapter.as_ref());
            }
        }
        match config {
            OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block => "core",
            _ => "protocol_not_compiled",
        }
    }
}
