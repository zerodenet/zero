use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::ProtocolMetadata;

use super::{
    BoundInbound, InboundAdapterContext, OutboundAdapterContext, OutboundLeafRuntime,
    UdpAdapterContext,
};
use crate::runtime::udp_flow::managed::{ManagedDatagramFlowHandler, ManagedStreamFlowHandler};
use crate::runtime::udp_flow::registered::UpstreamAssociationHandler;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

pub(crate) trait RegisteredProtocolCapability:
    ProtocolSupportCapability
    + InboundListenerCapability
    + TcpOutboundCapability
    + UdpFlowCapability
    + UdpPacketPathCapability
    + Send
    + Sync
    + std::fmt::Debug
{
}

impl<T> RegisteredProtocolCapability for T where
    T: ProtocolSupportCapability
        + InboundListenerCapability
        + TcpOutboundCapability
        + UdpFlowCapability
        + UdpPacketPathCapability
        + Send
        + Sync
        + std::fmt::Debug
{
}

pub(crate) trait ProtocolSupportCapability: ProtocolMetadata {
    fn name(&self) -> &'static str;
    fn feature_name(&self) -> &'static str;
    fn supports_inbound(&self, config: &InboundProtocolConfig) -> bool;
    fn supports_outbound(&self, config: &OutboundProtocolConfig) -> bool;
    fn has_inbound(&self) -> bool;
    fn has_outbound(&self) -> bool;

    fn on_config_reloaded(&self) {}
}

#[async_trait]
pub(crate) trait InboundListenerCapability {
    /// Bind the listener socket eagerly so port-in-use errors surface before
    /// the proxy announces "started".
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        super::defaults::bind_tcp_inbound(inbound).await
    }

    /// Spawn the inbound accept loop for `inbound` into `listeners`.
    fn spawn_inbound(
        &self,
        _ctx: InboundAdapterContext<'_>,
        _inbound: InboundConfig,
        _bound: BoundInbound,
        _shutdown_rx: tokio::sync::watch::Receiver<bool>,
        _listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
    }
}

#[async_trait]
pub(crate) trait TcpOutboundCapability {
    fn claims_outbound_leaf(&self, _leaf: &ResolvedLeafOutbound<'_>) -> bool {
        false
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        _leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        None
    }

    async fn connect_tcp(
        &self,
        _ctx: OutboundAdapterContext<'_>,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        Err(super::defaults::tcp_outbound_unsupported())
    }

    async fn apply_relay_hop(
        &self,
        _ctx: OutboundAdapterContext<'_>,
        stream: TcpRelayStream,
        _session: &Session,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let _ = stream;
        Err(super::defaults::relay_hop_unsupported())
    }
}

#[async_trait]
pub(crate) trait UdpFlowCapability {
    fn managed_datagram_udp_handler(&self) -> Option<Box<dyn ManagedDatagramFlowHandler>> {
        None
    }

    fn managed_stream_udp_handler(&self) -> Option<Box<dyn ManagedStreamFlowHandler>> {
        None
    }

    fn upstream_association_handler(&self) -> Option<Box<dyn UpstreamAssociationHandler>> {
        None
    }

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
        Err(super::defaults::udp_outbound_unsupported())
    }

    fn udp_relay_needs_two_streams(&self, _leaf: &ResolvedLeafOutbound<'_>) -> bool {
        false
    }

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
        Err(super::defaults::udp_two_stream_relay_unsupported())
    }

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
        Err(super::defaults::udp_relay_final_hop_unsupported())
    }
}

#[async_trait]
pub(crate) trait UdpPacketPathCapability {
    fn udp_packet_path_carrier_descriptor(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        None
    }

    async fn build_udp_packet_path(
        &self,
        _ctx: UdpAdapterContext<'_>,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        Err(super::defaults::packet_path_carrier_unsupported())
    }

    fn udp_datagram_source(
        &self,
        _leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::UdpDatagramSource> {
        None
    }
}
