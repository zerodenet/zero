//! Protocol adapter registry — eliminates per-protocol match arms in the proxy.
//!
//! Each protocol provides a `ProtocolAdapter` that knows its name, feature gate,
//! and how to validate its configuration.  The `ProtocolRegistry` collects
//! adapters at startup and replaces the hard-coded match statements in
//! `ProtocolInventory`.

use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::EngineError;

/// A protocol adapter registered in the proxy.
///
/// Implementations are behind `#[cfg(feature = "...")]` gates so only
/// compiled-in protocols appear in the registry.
pub trait ProtocolAdapter: Send + Sync {
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
/// Constructed at proxy startup.  Replaces the manual match arms in
/// `ProtocolInventory::supports_*` and `protocol_name` functions.
#[derive(Default)]
pub struct ProtocolRegistry {
    adapters: Vec<Box<dyn ProtocolAdapter>>,
}

impl ProtocolRegistry {
    pub fn new(adapters: Vec<Box<dyn ProtocolAdapter>>) -> Self {
        Self { adapters }
    }

    pub fn register(&mut self, adapter: Box<dyn ProtocolAdapter>) {
        self.adapters.push(adapter);
    }

    /// Names of all compiled-in inbound protocols.
    pub fn inbound_names(&self) -> Vec<&'static str> {
        self.adapters
            .iter()
            .filter(|a| a.has_inbound())
            .map(|a| a.name())
            .collect()
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
    pub fn validate_inbounds(&self, configs: &[zero_config::InboundConfig]) -> Result<(), EngineError> {
        for inbound in configs {
            if !self.supports_inbound(&inbound.protocol) {
                let name = inbound_protocol_label(&inbound.protocol);
                return Err(EngineError::CompiledFeatureDisabled {
                    kind: "inbound",
                    tag: inbound.tag.clone(),
                    protocol: name,
                    feature: "protocol-not-compiled",
                });
            }
        }
        Ok(())
    }

    /// Validate that every outbound in the config has a compiled-in adapter.
    pub fn validate_outbounds(&self, configs: &[zero_config::OutboundConfig]) -> Result<(), EngineError> {
        for outbound in configs {
            if !self.supports_outbound(&outbound.protocol) {
                let name = outbound_protocol_label(&outbound.protocol);
                return Err(EngineError::CompiledFeatureDisabled {
                    kind: "outbound",
                    tag: outbound.tag.clone(),
                    protocol: name,
                    feature: "protocol-not-compiled",
                });
            }
        }
        Ok(())
    }

    pub fn supports_inbound(&self, config: &InboundProtocolConfig) -> bool {
        self.adapters
            .iter()
            .any(|a| a.supports_inbound(config))
            || matches!(config, InboundProtocolConfig::Mixed { .. })
    }

    pub fn supports_outbound(&self, config: &OutboundProtocolConfig) -> bool {
        matches!(config, OutboundProtocolConfig::Direct | OutboundProtocolConfig::Block)
            || self.adapters.iter().any(|a| a.supports_outbound(config))
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn inbound_protocol_label(config: &InboundProtocolConfig) -> &'static str {
    match config {
        InboundProtocolConfig::Socks5 { .. } => "socks5",
        InboundProtocolConfig::HttpConnect => "http-connect",
        InboundProtocolConfig::Mixed { .. } => "mixed",
        InboundProtocolConfig::Vless { .. } => "vless",
        InboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
        InboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
        InboundProtocolConfig::Trojan { .. } => "trojan",
    }
}

fn outbound_protocol_label(config: &OutboundProtocolConfig) -> &'static str {
    match config {
        OutboundProtocolConfig::Direct => "direct",
        OutboundProtocolConfig::Block => "block",
        OutboundProtocolConfig::Socks5 { .. } => "socks5",
        OutboundProtocolConfig::Vless { .. } => "vless",
        OutboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
        OutboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
        OutboundProtocolConfig::Trojan { .. } => "trojan",
    }
}
