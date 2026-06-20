use std::fmt;
use std::sync::Arc;

use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::protocol_adapter::{BoundInbound, ProtocolAdapter};
use crate::protocol_capability::{protocol_capability, protocol_descriptor};

/// Registry of all compiled-in protocol adapters.
///
/// Constructed at proxy startup via `build_registry()`. Replaces the manual
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
mod tests {
    use super::*;

    fn inbound_protocol_name(config: &InboundProtocolConfig) -> &'static str {
        match config {
            InboundProtocolConfig::Socks5 { .. } => "socks5",
            InboundProtocolConfig::HttpConnect => "http_connect",
            InboundProtocolConfig::Mixed { .. } => "mixed",
            InboundProtocolConfig::Vless { .. } => "vless",
            InboundProtocolConfig::Hysteria2 { .. } => "hysteria2",
            InboundProtocolConfig::Shadowsocks { .. } => "shadowsocks",
            InboundProtocolConfig::Trojan { .. } => "trojan",
            InboundProtocolConfig::Vmess { .. } => "vmess",
            InboundProtocolConfig::Direct { .. } => "direct",
            InboundProtocolConfig::Mieru { .. } => "mieru",
        }
    }

    fn outbound_leaf_name(leaf: &ResolvedLeafOutbound<'_>) -> &'static str {
        match leaf {
            ResolvedLeafOutbound::Direct { .. } => "direct",
            ResolvedLeafOutbound::Block { .. } => "block",
            ResolvedLeafOutbound::Socks5 { .. } => "socks5",
            ResolvedLeafOutbound::Vless { .. } => "vless",
            ResolvedLeafOutbound::Hysteria2 { .. } => "hysteria2",
            ResolvedLeafOutbound::Shadowsocks { .. } => "shadowsocks",
            ResolvedLeafOutbound::Trojan { .. } => "trojan",
            ResolvedLeafOutbound::Vmess { .. } => "vmess",
            ResolvedLeafOutbound::Mieru { .. } => "mieru",
        }
    }

    fn compiled_in_inbound_configs() -> Vec<InboundProtocolConfig> {
        let mut configs = vec![InboundProtocolConfig::Direct {
            target: None,
            port: None,
        }];

        #[cfg(feature = "socks5")]
        configs.push(InboundProtocolConfig::Socks5 { users: Vec::new() });
        #[cfg(feature = "http_connect")]
        configs.push(InboundProtocolConfig::HttpConnect);
        #[cfg(feature = "mixed")]
        configs.push(InboundProtocolConfig::Mixed {
            socks5_users: Vec::new(),
        });
        #[cfg(feature = "vless")]
        configs.push(InboundProtocolConfig::Vless {
            users: Vec::new(),
            tls: None,
            reality: None,
            ws: None,
            grpc: None,
            h2: None,
            http_upgrade: None,
            fallback: None,
            quic: None,
            split_http: None,
        });
        #[cfg(feature = "hysteria2")]
        configs.push(InboundProtocolConfig::Hysteria2 {
            password: "password".to_string(),
            cert_path: None,
            key_path: None,
            up_bps: None,
            down_bps: None,
        });
        #[cfg(feature = "shadowsocks")]
        configs.push(InboundProtocolConfig::Shadowsocks {
            password: "password".to_string(),
            cipher: "chacha20-ietf-poly1305".to_string(),
            up_bps: None,
            down_bps: None,
        });
        #[cfg(feature = "trojan")]
        configs.push(InboundProtocolConfig::Trojan {
            password: "password".to_string(),
            sni: None,
            tls: None,
            up_bps: None,
            down_bps: None,
        });
        #[cfg(feature = "vmess")]
        configs.push(InboundProtocolConfig::Vmess {
            users: Vec::new(),
            tls: None,
            ws: None,
            grpc: None,
        });
        #[cfg(feature = "mieru")]
        configs.push(InboundProtocolConfig::Mieru { users: Vec::new() });

        configs
    }

    fn compiled_in_outbound_leaves<'a>() -> Vec<(ResolvedLeafOutbound<'a>, usize)> {
        let mut leaves = vec![
            (
                ResolvedLeafOutbound::Direct {
                    tag: Some("direct"),
                },
                1,
            ),
            (ResolvedLeafOutbound::Block { tag: Some("block") }, 0),
        ];

        #[cfg(feature = "socks5")]
        leaves.push((
            ResolvedLeafOutbound::Socks5 {
                tag: "socks5",
                server: "127.0.0.1",
                port: 1080,
                username: None,
                password: None,
            },
            1,
        ));
        #[cfg(feature = "vless")]
        leaves.push((
            ResolvedLeafOutbound::Vless {
                tag: "vless",
                server: "127.0.0.1",
                port: 443,
                id: "00000000-0000-0000-0000-000000000000",
                flow: None,
                mux_concurrency: None,
                mux_idle_timeout_secs: None,
                tls: None,
                reality: None,
                ws: None,
                grpc: None,
                h2: None,
                http_upgrade: None,
                split_http: None,
                quic: None,
            },
            1,
        ));
        #[cfg(feature = "hysteria2")]
        leaves.push((
            ResolvedLeafOutbound::Hysteria2 {
                tag: "hysteria2",
                server: "127.0.0.1",
                port: 443,
                password: "password",
                insecure: false,
                client_fingerprint: None,
            },
            1,
        ));
        #[cfg(feature = "shadowsocks")]
        leaves.push((
            ResolvedLeafOutbound::Shadowsocks {
                tag: "shadowsocks",
                server: "127.0.0.1",
                port: 8388,
                password: "password",
                cipher: "chacha20-ietf-poly1305",
            },
            1,
        ));
        #[cfg(feature = "trojan")]
        leaves.push((
            ResolvedLeafOutbound::Trojan {
                tag: "trojan",
                server: "127.0.0.1",
                port: 443,
                password: "password",
                sni: None,
                insecure: false,
                client_fingerprint: None,
            },
            1,
        ));
        #[cfg(feature = "vmess")]
        leaves.push((
            ResolvedLeafOutbound::Vmess {
                tag: "vmess",
                server: "127.0.0.1",
                port: 443,
                id: "00000000-0000-0000-0000-000000000000",
                cipher: "aes-128-gcm",
                mux_concurrency: None,
                mux_idle_timeout_secs: None,
                tls: None,
                ws: None,
                grpc: None,
            },
            1,
        ));
        #[cfg(feature = "mieru")]
        leaves.push((
            ResolvedLeafOutbound::Mieru {
                tag: "mieru",
                server: "127.0.0.1",
                port: 8964,
                username: "",
                password: "password",
            },
            1,
        ));

        leaves
    }

    #[test]
    fn compiled_in_inbound_variants_have_exactly_one_registered_adapter() {
        let registry = crate::adapters::build_registry();

        for config in compiled_in_inbound_configs() {
            let claim_count = registry
                .adapters
                .iter()
                .filter(|adapter| adapter.supports_inbound(&config))
                .count();
            assert_eq!(
                claim_count,
                1,
                "{} inbound config should be claimed by exactly one adapter",
                inbound_protocol_name(&config)
            );
            assert!(
                registry.find_inbound(&config).is_ok(),
                "{} inbound config should resolve through ProtocolRegistry::find_inbound",
                inbound_protocol_name(&config)
            );
        }
    }

    #[test]
    fn compiled_in_outbound_leaf_variants_have_expected_adapter_claims() {
        let registry = crate::adapters::build_registry();

        for (leaf, expected_claims) in compiled_in_outbound_leaves() {
            let claim_count = registry
                .adapters
                .iter()
                .filter(|adapter| adapter.claims_outbound_leaf(&leaf))
                .count();
            assert_eq!(
                claim_count,
                expected_claims,
                "{} outbound leaf should have {expected_claims} adapter claim(s)",
                outbound_leaf_name(&leaf)
            );

            let resolved = registry.find_outbound_leaf(&leaf);
            assert_eq!(
                resolved.is_ok(),
                expected_claims == 1,
                "{} outbound leaf registry lookup result did not match claim policy",
                outbound_leaf_name(&leaf)
            );
        }
    }
}
