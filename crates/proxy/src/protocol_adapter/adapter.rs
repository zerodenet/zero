use std::fmt;

use async_trait::async_trait;

use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::ProtocolMetadata;

use super::{
    defaults, BoundInbound, InboundAdapterContext, OutboundAdapterContext, OutboundLeafRuntime,
    UdpAdapterContext,
};
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

/// A protocol adapter registered in the proxy.
///
/// Implementations are behind `#[cfg(feature = "...")]` gates so only
/// compiled-in protocols appear in the registry.
#[async_trait]
pub(crate) trait ProtocolAdapter: ProtocolMetadata + Send + Sync + fmt::Debug {
    /// Bind the listener socket for `config` eagerly so port-in-use
    /// errors surface before the proxy announces "started".
    ///
    /// Defaults to a plain TCP bind on the listen address. QUIC-based
    /// protocols (VLESS/QUIC, Hysteria2) override to create a QUIC endpoint,
    /// reading their own cert/key config - the runtime never touches those
    /// fields.
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        defaults::bind_tcp_inbound(inbound).await
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
    /// adapter returns `true` only for its own resolved leaf variant.
    fn claims_outbound_leaf(&self, _leaf: &ResolvedLeafOutbound<'_>) -> bool {
        false
    }

    /// Return the neutral runtime facts for a leaf this adapter owns.
    fn outbound_leaf_runtime<'a>(
        &self,
        _leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        None
    }

    /// Establish a TCP outbound connection for the resolved leaf.
    ///
    /// The adapter extracts its own variant from `leaf`, reads its own
    /// protocol-private fields (password/cipher/uuid - the runtime never
    /// touches those), performs the transport + protocol handshake, and
    /// returns the established outbound. Defaults to "not supported" so
    /// inbound-only adapters (e.g. HTTP CONNECT) need not override.
    ///
    /// This is the outbound mirror of [`crate::runtime::inbound_protocol::InboundProtocol`]:
    /// the runtime dispatches via [`ProtocolRegistry::find_outbound_leaf`]
    /// instead of matching on `ResolvedLeafOutbound`.
    async fn connect_tcp(
        &self,
        _ctx: OutboundAdapterContext<'_>,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        Err(defaults::tcp_outbound_unsupported())
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
        _ctx: OutboundAdapterContext<'_>,
        stream: TcpRelayStream,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let _ = stream;
        Err(defaults::relay_hop_unsupported())
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
        _ctx: UdpAdapterContext<'_>,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
        _payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        Err(defaults::udp_outbound_unsupported())
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
        _ctx: InboundAdapterContext<'_>,
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
        _ctx: UdpAdapterContext<'_>,
        _session: &Session,
        _chain: Vec<ResolvedLeafOutbound<'_>>,
        _payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        Err(defaults::udp_two_stream_relay_unsupported())
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
        _ctx: UdpAdapterContext<'_>,
        _session: &Session,
        carrier: crate::transport::RelayCarrier,
        _leaf: &ResolvedLeafOutbound<'_>,
        _payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        let _ = carrier;
        Err(defaults::udp_relay_final_hop_unsupported())
    }

    /// If this leaf can serve as a UDP packet-path carrier (relay-chain first
    /// hop that provides a raw send/recv channel), return its identity
    /// descriptor (cache key + endpoint). Cheap; used for cache lookup before
    /// [`Self::build_udp_packet_path`] dials.
    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_descriptor(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::PacketPathCarrierDescriptor> {
        None
    }

    /// Owned snapshot of the carrier for flow status/result reporting.
    ///
    /// Only carrier-capable adapters override this. The runtime uses it when a
    /// relay chain caches a packet-path carrier and needs to keep a stable
    /// owned representation in `UdpFlowOutbound`.
    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_snapshot(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
        None
    }

    /// Build the concrete packet-path carrier for this leaf (dial + establish).
    /// Only called on a cache miss. Defaults to "not supported".
    #[cfg(feature = "shadowsocks")]
    async fn build_udp_packet_path(
        &self,
        _ctx: UdpAdapterContext<'_>,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<std::sync::Arc<dyn crate::protocol_runtime::udp::PacketPathCarrier>, EngineError>
    {
        Err(defaults::packet_path_carrier_unsupported())
    }

    /// If this leaf can be a UDP packet-path datagram source (relay-chain final
    /// hop that encodes its datagram through a carrier), return its params.
    /// `None` for protocols that cannot serve this role.
    #[cfg(feature = "shadowsocks")]
    fn udp_datagram_source<'a>(
        &self,
        _leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<crate::protocol_runtime::udp::UdpDatagramSource<'a>> {
        None
    }
}
