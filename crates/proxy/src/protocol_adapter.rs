//! Protocol adapter registry — eliminates per-protocol match arms in the proxy.
//!
//! Each protocol provides a `ProtocolAdapter` that knows its name, feature gate,
//! and how to validate its configuration.  The `ProtocolRegistry` collects
//! adapters at startup and replaces the hard-coded match statements in
//! `ProtocolInventory`.

use std::fmt;
use std::sync::Arc;

use async_trait::async_trait;

use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::EngineError;
use zero_traits::{ProtocolMetadata, TransportKind};

use crate::protocol_capability::{protocol_capability, protocol_descriptor};

/// A pre-bound inbound listener — TCP or QUIC.
///
/// Produced by [`ProtocolAdapter::bind_inbound`] **before** the accept loop
/// spawns, so port conflicts surface immediately via `?` rather than surfacing
/// later through `JoinSet::join_next()`. The bind logic stays owned by the
/// adapter (which reads its own protocol config) instead of leaking protocol
/// private fields into the runtime dispatch.
pub(crate) enum BoundInbound {
    Tcp(zero_platform_tokio::TokioListener),
    #[cfg(any(feature = "vless", feature = "hysteria2"))]
    Quic(crate::transport::QuicInbound),
}

impl BoundInbound {
    /// Unwrap into a TCP listener. Panics if the variant is QUIC —
    /// indicates a dispatch mismatch (bind vs spawn disagree), which
    /// should never happen since both go through the same adapter.
    pub(crate) fn into_tcp(self) -> zero_platform_tokio::TokioListener {
        match self {
            Self::Tcp(l) => l,
            #[cfg(any(feature = "vless", feature = "hysteria2"))]
            Self::Quic(_) => {
                panic!("into_tcp: got QUIC listener, expected TCP (dispatch mismatch)")
            }
            #[cfg(not(any(feature = "vless", feature = "hysteria2")))]
            _ => unreachable!(),
        }
    }
}

/// A protocol adapter registered in the proxy.
///
/// Implementations are behind `#[cfg(feature = "...")]` gates so only
/// compiled-in protocols appear in the registry.
#[async_trait]
pub trait ProtocolAdapter: ProtocolMetadata + Send + Sync + fmt::Debug {
    /// Bind the listener socket for `config` eagerly so port-in-use
    /// errors surface before the proxy announces "started".
    ///
    /// Defaults to a plain TCP bind on the listen address. QUIC-based
    /// protocols (VLESS/QUIC, Hysteria2) override to create a QUIC endpoint,
    /// reading their own cert/key config — the runtime never touches those
    /// fields.
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        let tcp = zero_platform_tokio::TokioListener::bind(&listen)
            .await
            .map_err(EngineError::Io)?;
        Ok(BoundInbound::Tcp(tcp))
    }

    /// Transport kind the inbound listener uses for `config`.
    ///
    /// Defaults to [`TransportKind::Tcp`]; QUIC-based protocols (VLESS/QUIC,
    /// Hysteria2) override. This lets the runtime dispatch bind/spawn
    /// decisions without re-reading the protocol's private config fields.
    fn inbound_transport_kind(&self, _config: &InboundProtocolConfig) -> TransportKind {
        TransportKind::Tcp
    }

    /// Protocol name used in config `"type"` field and exported status.
    fn name(&self) -> &'static str;

    /// Cargo feature that gates this protocol (e.g. `"socks5"`).
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
        if cfg!(feature = "mixed") {
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
        if cfg!(feature = "mixed")
            && !descriptors
                .iter()
                .any(|descriptor| descriptor.protocol == "mixed")
        {
            descriptors.push(protocol_descriptor("mixed", "mixed"));
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
            return "mixed";
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
        if matches!(inbound.protocol, InboundProtocolConfig::Mixed { .. }) {
            let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
            let tcp = zero_platform_tokio::TokioListener::bind(&listen)
                .await
                .map_err(EngineError::Io)?;
            return Ok(BoundInbound::Tcp(tcp));
        }
        let name = self.inbound_protocol_label(&inbound.protocol);
        Err(EngineError::CompiledFeatureDisabled {
            kind: "inbound",
            tag: inbound.tag.clone(),
            protocol: name,
            feature: self.inbound_protocol_feature_name(&inbound.protocol),
        })
    }

    /// Transport kind for an inbound config via its registered adapter.
    pub fn inbound_transport_kind(&self, config: &InboundProtocolConfig) -> TransportKind {
        self.adapters
            .iter()
            .find(|a| a.supports_inbound(config))
            .map(|a| a.inbound_transport_kind(config))
            .unwrap_or(TransportKind::Tcp)
    }
}
