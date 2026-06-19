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
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::ProtocolMetadata;

use crate::protocol_capability::{protocol_capability, protocol_descriptor};
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

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
    #[cfg(any(feature = "vless", feature = "hysteria2"))]
    pub(crate) fn into_tcp(self) -> zero_platform_tokio::TokioListener {
        match self {
            Self::Tcp(l) => l,
            Self::Quic(_) => {
                panic!("into_tcp: got QUIC listener, expected TCP (dispatch mismatch)")
            }
        }
    }

    #[cfg(not(any(feature = "vless", feature = "hysteria2")))]
    pub(crate) fn into_tcp(self) -> zero_platform_tokio::TokioListener {
        match self {
            Self::Tcp(l) => l,
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
        _source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        let listen = format!("{}:{}", inbound.listen.address, inbound.listen.port);
        let tcp = zero_platform_tokio::TokioListener::bind(&listen)
            .await
            .map_err(EngineError::Io)?;
        Ok(BoundInbound::Tcp(tcp))
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

    /// Whether this adapter owns the given resolved outbound leaf.
    ///
    /// Single dispatch probe: the runtime calls this to find the adapter that
    /// handles a [`ResolvedLeafOutbound`] instead of matching on the protocol
    /// enum. Each adapter claims exactly its own variant, e.g. the SOCKS5
    /// adapter returns `true` only for `ResolvedLeafOutbound::Socks5 { .. }`.
    fn claims_outbound_leaf(&self, _leaf: &ResolvedLeafOutbound<'_>) -> bool {
        false
    }

    /// Establish a TCP outbound connection for the resolved leaf.
    ///
    /// The adapter extracts its own variant from `leaf`, reads its own
    /// protocol-private fields (password/cipher/uuid — the runtime never
    /// touches those), performs the transport + protocol handshake, and
    /// returns the established outbound. Defaults to "not supported" so
    /// inbound-only adapters (e.g. HTTP CONNECT) need not override.
    ///
    /// This is the outbound mirror of [`crate::runtime::inbound_protocol::InboundProtocol`]:
    /// the runtime dispatches via [`ProtocolRegistry::find_outbound_leaf`]
    /// instead of matching on `ResolvedLeafOutbound`.
    async fn connect_tcp(
        &self,
        _proxy: &Proxy,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        Err(TcpOutboundFailure {
            stage: "no_tcp_outbound",
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "this adapter does not provide a TCP outbound",
            )),
            upstream_endpoint: None,
        })
    }

    /// Apply this protocol's handshake to an existing stream (relay chain hop).
    ///
    /// For relay chains, the first hop uses [`Self::connect_tcp`] (dial +
    /// handshake); subsequent hops receive an already-connected stream from
    /// the previous hop and only run their protocol handshake over it.
    /// Adapters that cannot serve as a relay hop leave the default
    /// ("not supported") impl.
    async fn apply_relay_hop(
        &self,
        _proxy: &Proxy,
        stream: TcpRelayStream,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let _ = stream;
        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "this adapter does not support relay hop",
        )))
    }

    /// Start a UDP outbound flow for the resolved leaf.
    ///
    /// The adapter extracts its own variant from `leaf` and drives its
    /// per-protocol UDP manager on `dispatch` (each protocol owns a manager
    /// field on [`crate::runtime::udp_dispatch::UdpDispatch`]). The runtime
    /// dispatches via [`ProtocolRegistry::find_outbound_leaf`] instead of
    /// matching on the protocol enum. Defaults to "not supported".
    async fn start_udp_flow(
        &self,
        _dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        _proxy: &Proxy,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
        _payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        Err(crate::runtime::udp_dispatch::FlowFailure {
            stage: "no_udp_outbound",
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "this adapter does not provide a UDP outbound",
            )),
            upstream: None,
        })
    }

    /// Spawn the inbound accept loop for `inbound` into `listeners`.
    ///
    /// The adapter owns the full inbound lifecycle from bind to run: it clones
    /// the proxy, extracts the listener from `bound` (calling `into_tcp()` for
    /// TCP-only protocols, keeping QUIC for VLESS/Hysteria2), and spawns its
    /// `run_<protocol>_listener_with_bound` task. The runtime dispatches via
    /// [`ProtocolRegistry::find_inbound`] instead of matching on the protocol
    /// enum. Default is a no-op (inbound-only adapters override).
    fn spawn_inbound(
        &self,
        _proxy: &Proxy,
        _inbound: InboundConfig,
        _bound: BoundInbound,
        _shutdown_rx: tokio::sync::watch::Receiver<bool>,
        _listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
    }

    /// Whether the UDP relay chain final hop needs the two-stream XHTTP path.
    ///
    /// Only the VLESS adapter overrides this (returns `true` for legacy
    /// split_http packet-up / stream-up modes). The runtime checks this
    /// *before* running the relay prefix so it can dial two carrier streams.
    fn udp_relay_needs_two_streams(&self, _leaf: &ResolvedLeafOutbound<'_>) -> bool {
        false
    }

    /// Drive the two-stream XHTTP UDP relay path (VLESS legacy split_http).
    ///
    /// The adapter owns the full path: it runs the relay prefix twice (POST +
    /// GET carrier), builds the split-HTTP pair, and establishes the VLESS UDP
    /// upstream. Only the VLESS adapter overrides this.
    async fn start_udp_relay_two_stream(
        &self,
        _dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        _proxy: &Proxy,
        _session: &Session,
        _chain: Vec<ResolvedLeafOutbound<'_>>,
        _payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        Err(crate::runtime::udp_dispatch::FlowFailure {
            stage: "no_two_stream_relay",
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "this adapter does not support two-stream UDP relay",
            )),
            upstream: None,
        })
    }

    /// Establish the UDP final hop over a carrier stream from the relay prefix.
    ///
    /// The adapter receives the carrier produced by `dispatch_tcp_relay_prefix`
    /// and runs its protocol's UDP-over-relay logic (build transport over the
    /// stream, or pass the stream to its chain manager). The runtime dispatches
    /// via [`ProtocolRegistry::find_outbound_leaf`] instead of matching on the
    /// protocol enum. Defaults to "not supported".
    async fn start_udp_relay_final_hop(
        &self,
        _dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        _proxy: &Proxy,
        _session: &Session,
        carrier: crate::transport::RelayCarrier,
        _leaf: &ResolvedLeafOutbound<'_>,
        _payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        let _ = carrier;
        Err(crate::runtime::udp_dispatch::FlowFailure {
            stage: "no_udp_relay_final_hop",
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "this adapter does not support UDP relay final hop",
            )),
            upstream: None,
        })
    }

    /// If this leaf can serve as a UDP packet-path carrier (relay-chain first
    /// hop that provides a raw send/recv channel), return its identity
    /// descriptor (cache key + endpoint). Cheap; used for cache lookup before
    /// [`Self::build_udp_packet_path`] dials.
    fn udp_packet_path_carrier_descriptor(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_dispatch::PacketPathCarrierDescriptor> {
        None
    }

    /// Owned snapshot of the carrier for flow status/result reporting.
    ///
    /// Only carrier-capable adapters override this. The runtime uses it when a
    /// relay chain caches a packet-path carrier and needs to keep a stable
    /// owned representation in `UdpFlowOutbound`.
    fn udp_packet_path_carrier_snapshot(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_associate::sessions::UdpPacketPathCarrier> {
        None
    }

    /// Build the concrete packet-path carrier for this leaf (dial + establish).
    /// Only called on a cache miss. Defaults to "not supported".
    async fn build_udp_packet_path(
        &self,
        _proxy: &Proxy,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<std::sync::Arc<dyn crate::runtime::udp_dispatch::PacketPathCarrier>, EngineError>
    {
        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "this adapter does not provide a UDP packet-path carrier",
        )))
    }

    /// If this leaf can be a UDP packet-path datagram source (relay-chain final
    /// hop that encodes its datagram through a carrier), return its params.
    /// `None` for protocols that cannot serve this role.
    fn udp_datagram_source<'a>(
        &self,
        _leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<crate::runtime::udp_dispatch::UdpDatagramSource<'a>> {
        None
    }
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
}
