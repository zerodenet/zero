use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};

use super::ProtocolRegistry;

impl ProtocolRegistry {
    pub(crate) fn supports_inbound(&self, config: &InboundProtocolConfig) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.support.supports_inbound(config))
    }

    pub(crate) fn supports_outbound(&self, config: &OutboundProtocolConfig) -> bool {
        matches!(
            config,
            OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block
        ) || self
            .entries
            .iter()
            .any(|entry| entry.support.supports_outbound(config))
    }

    /// Human-readable label for an inbound protocol config.
    pub(crate) fn inbound_protocol_label(&self, config: &InboundProtocolConfig) -> &'static str {
        for entry in &self.entries {
            if entry.support.supports_inbound(config) {
                return entry.support.name();
            }
        }
        config.protocol_name()
    }

    /// Cargo feature name needed to compile this inbound protocol.
    pub(crate) fn inbound_protocol_feature_name(
        &self,
        config: &InboundProtocolConfig,
    ) -> &'static str {
        for entry in &self.entries {
            if entry.support.supports_inbound(config) {
                return entry.support.feature_name();
            }
        }
        let protocol = config.protocol_name();
        if protocol == "direct" {
            "core"
        } else {
            protocol
        }
    }

    /// Human-readable label for an outbound protocol config.
    pub(crate) fn outbound_protocol_label(&self, config: &OutboundProtocolConfig) -> &'static str {
        for entry in &self.entries {
            if entry.support.supports_outbound(config) {
                return entry.support.name();
            }
        }
        config.protocol_name()
    }

    /// Cargo feature name needed to compile this outbound protocol.
    pub(crate) fn outbound_protocol_feature_name(
        &self,
        config: &OutboundProtocolConfig,
    ) -> &'static str {
        for entry in &self.entries {
            if entry.support.supports_outbound(config) {
                return entry.support.feature_name();
            }
        }
        let protocol = config.protocol_name();
        if matches!(protocol, "direct" | "block") {
            "core"
        } else {
            protocol
        }
    }
}
