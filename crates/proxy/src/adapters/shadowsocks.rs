use std::sync::Arc;

use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::common::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    proxy_leaf_runtime, BoundInbound, InboundAdapterContext, InboundListenerCapability,
    OutboundAdapterContext, OutboundLeafRuntime, ProtocolSupportCapability, TcpOutboundCapability,
    UdpAdapterContext, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::orchestration::TcpPathCategory;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

#[cfg(feature = "shadowsocks")]
mod inbound;
#[cfg(feature = "shadowsocks")]
mod tcp;
#[cfg(feature = "shadowsocks")]
pub(crate) mod udp;

#[cfg(feature = "shadowsocks")]
#[derive(Debug)]
pub(crate) struct ShadowsocksAdapter;

#[cfg(feature = "shadowsocks")]
impl NamedProtocolAdapter for ShadowsocksAdapter {
    const PROTOCOL_NAME: &'static str = "shadowsocks";
    const FEATURE_NAME: &'static str = "shadowsocks";
}

#[cfg(feature = "shadowsocks")]
#[async_trait]
impl UdpPacketPathCapability for ShadowsocksAdapter {
    fn udp_packet_path_carrier_descriptor(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        self.udp_packet_path_carrier_descriptor_impl(leaf)
    }

    async fn build_udp_packet_path(
        &self,
        ctx: UdpAdapterContext<'_>,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>, EngineError>
    {
        self.build_udp_packet_path_impl(ctx.proxy(), leaf).await
    }

    fn udp_datagram_source(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::UdpDatagramSource> {
        self.udp_datagram_source_impl(leaf)
    }
}

#[cfg(feature = "shadowsocks")]
#[async_trait]
impl UdpFlowCapability for ShadowsocksAdapter {
    fn managed_datagram_udp_handler(&self) -> Option<Box<dyn ManagedDatagramFlowHandler>> {
        Some(udp::managed_datagram_handler())
    }

    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        self.start_udp_flow_impl(dispatch, ctx.proxy(), session, leaf, payload)
            .await
    }
}

#[cfg(feature = "shadowsocks")]
impl InboundListenerCapability for ShadowsocksAdapter {
    fn spawn_inbound(
        &self,
        ctx: InboundAdapterContext<'_>,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        self.spawn_inbound_impl(ctx.proxy(), inbound, bound, shutdown_rx, listeners);
    }
}

#[cfg(feature = "shadowsocks")]
#[async_trait]
impl TcpOutboundCapability for ShadowsocksAdapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        named_protocol_claims_runtime_leaf::<Self>(leaf)
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        proxy_leaf_runtime(leaf, TcpPathCategory::Session)
    }

    async fn connect_tcp(
        &self,
        ctx: OutboundAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        self.connect_tcp_impl(ctx.proxy(), session, leaf).await
    }

    async fn apply_relay_hop(
        &self,
        _ctx: OutboundAdapterContext<'_>,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        self.apply_relay_hop_impl(stream, session, leaf).await
    }
}

#[cfg(feature = "shadowsocks")]
impl ProtocolSupportCapability for ShadowsocksAdapter {
    fn name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::PROTOCOL_NAME
    }
    fn feature_name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::FEATURE_NAME
    }
    fn has_inbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_INBOUND
    }
    fn has_outbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_OUTBOUND
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        named_protocol_supports_inbound::<Self>(c)
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        named_protocol_supports_outbound::<Self>(c)
    }
}

#[cfg(feature = "shadowsocks")]
impl ProtocolMetadata for ShadowsocksAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::shadowsocks::ShadowsocksProtocol.descriptor()
    }
}
