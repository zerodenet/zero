use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::ProtocolMetadata;

use super::{
    BoundInbound, InboundAdapterContext, OutboundAdapterContext, OutboundLeafRuntime,
    ProtocolAdapter, UdpAdapterContext,
};
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
}

#[async_trait]
pub(crate) trait InboundListenerCapability {
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError>;

    fn spawn_inbound(
        &self,
        ctx: InboundAdapterContext<'_>,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    );
}

#[async_trait]
pub(crate) trait TcpOutboundCapability {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool;

    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>>;

    async fn connect_tcp(
        &self,
        ctx: OutboundAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>;

    async fn apply_relay_hop(
        &self,
        ctx: OutboundAdapterContext<'_>,
        stream: TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<TcpRelayStream, EngineError>;
}

#[async_trait]
pub(crate) trait UdpFlowCapability {
    async fn start_udp_flow(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    >;

    fn udp_relay_needs_two_streams(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool;

    async fn start_udp_relay_two_stream(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    >;

    async fn start_udp_relay_final_hop(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    >;
}

#[async_trait]
pub(crate) trait UdpPacketPathCapability {
    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_descriptor(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::PacketPathCarrierDescriptor>;

    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_snapshot(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier>;

    #[cfg(feature = "shadowsocks")]
    async fn build_udp_packet_path(
        &self,
        ctx: UdpAdapterContext<'_>,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<std::sync::Arc<dyn crate::protocol_runtime::udp::PacketPathCarrier>, EngineError>;

    #[cfg(feature = "shadowsocks")]
    fn udp_datagram_source<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<crate::protocol_runtime::udp::UdpDatagramSource<'a>>;
}

#[async_trait]
impl<T> InboundListenerCapability for T
where
    T: ProtocolAdapter + ?Sized,
{
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        ProtocolAdapter::bind_inbound(self, inbound, source_dir).await
    }

    fn spawn_inbound(
        &self,
        ctx: InboundAdapterContext<'_>,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        ProtocolAdapter::spawn_inbound(self, ctx, inbound, bound, shutdown_rx, listeners);
    }
}

#[async_trait]
impl<T> TcpOutboundCapability for T
where
    T: ProtocolAdapter + ?Sized,
{
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        ProtocolAdapter::claims_outbound_leaf(self, leaf)
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        ProtocolAdapter::outbound_leaf_runtime(self, leaf)
    }

    async fn connect_tcp(
        &self,
        ctx: OutboundAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        ProtocolAdapter::connect_tcp(self, ctx, session, leaf).await
    }

    async fn apply_relay_hop(
        &self,
        ctx: OutboundAdapterContext<'_>,
        stream: TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        ProtocolAdapter::apply_relay_hop(self, ctx, stream, session, leaf).await
    }
}

#[async_trait]
impl<T> UdpFlowCapability for T
where
    T: ProtocolAdapter + ?Sized,
{
    async fn start_udp_flow(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        ProtocolAdapter::start_udp_flow(self, dispatch, ctx, session, leaf, payload).await
    }

    fn udp_relay_needs_two_streams(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        ProtocolAdapter::udp_relay_needs_two_streams(self, leaf)
    }

    async fn start_udp_relay_two_stream(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        ProtocolAdapter::start_udp_relay_two_stream(self, dispatch, ctx, session, chain, payload)
            .await
    }

    async fn start_udp_relay_final_hop(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        ProtocolAdapter::start_udp_relay_final_hop(
            self, dispatch, ctx, session, carrier, leaf, payload,
        )
        .await
    }
}

#[async_trait]
impl<T> UdpPacketPathCapability for T
where
    T: ProtocolAdapter + ?Sized,
{
    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_descriptor(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::PacketPathCarrierDescriptor> {
        ProtocolAdapter::udp_packet_path_carrier_descriptor(self, leaf)
    }

    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_snapshot(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
        ProtocolAdapter::udp_packet_path_carrier_snapshot(self, leaf)
    }

    #[cfg(feature = "shadowsocks")]
    async fn build_udp_packet_path(
        &self,
        ctx: UdpAdapterContext<'_>,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<std::sync::Arc<dyn crate::protocol_runtime::udp::PacketPathCarrier>, EngineError>
    {
        ProtocolAdapter::build_udp_packet_path(self, ctx, leaf).await
    }

    #[cfg(feature = "shadowsocks")]
    fn udp_datagram_source<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<crate::protocol_runtime::udp::UdpDatagramSource<'a>> {
        ProtocolAdapter::udp_datagram_source(self, leaf)
    }
}
