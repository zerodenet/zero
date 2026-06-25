use std::sync::Arc;

use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::common::proxy_leaf_runtime;
use crate::protocol_adapter::{
    BoundInbound, InboundAdapterContext, InboundListenerCapability, OutboundAdapterContext,
    OutboundLeafRuntime, ProtocolAdapter, ProtocolSupportCapability, TcpOutboundCapability,
    UdpAdapterContext,
};
use crate::runtime::orchestration::TcpPathCategory;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

#[cfg(feature = "hysteria2")]
mod inbound;
#[cfg(feature = "hysteria2")]
mod tcp;
#[cfg(feature = "hysteria2")]
mod udp;

#[cfg(feature = "hysteria2")]
#[derive(Debug)]
pub(crate) struct Hysteria2Adapter;

#[cfg(feature = "hysteria2")]
#[async_trait]
impl ProtocolAdapter for Hysteria2Adapter {
    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_descriptor(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::PacketPathCarrierDescriptor> {
        self.udp_packet_path_carrier_descriptor_impl(leaf)
    }

    #[cfg(feature = "shadowsocks")]
    fn udp_packet_path_carrier_snapshot(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
        self.udp_packet_path_carrier_snapshot_impl(leaf)
    }

    #[cfg(feature = "shadowsocks")]
    async fn build_udp_packet_path(
        &self,
        _ctx: UdpAdapterContext<'_>,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn crate::protocol_runtime::udp::PacketPathCarrier>, EngineError> {
        self.build_udp_packet_path_impl(leaf).await
    }

    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        _ctx: UdpAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        self.start_udp_flow_impl(dispatch, session, leaf, payload)
            .await
    }
}

#[cfg(feature = "hysteria2")]
#[async_trait]
impl InboundListenerCapability for Hysteria2Adapter {
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        self.bind_inbound_impl(inbound, source_dir).await
    }

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

#[cfg(feature = "hysteria2")]
#[async_trait]
impl TcpOutboundCapability for Hysteria2Adapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Hysteria2 { .. })
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        proxy_leaf_runtime(leaf, TcpPathCategory::TransportSession)
    }

    async fn connect_tcp(
        &self,
        ctx: OutboundAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        self.connect_tcp_impl(ctx.proxy(), session, leaf).await
    }
}

#[cfg(feature = "hysteria2")]
impl ProtocolSupportCapability for Hysteria2Adapter {
    fn name(&self) -> &'static str {
        "hysteria2"
    }
    fn feature_name(&self) -> &'static str {
        "hysteria2"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Hysteria2 { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Hysteria2 { .. })
    }
}

#[cfg(feature = "hysteria2")]
impl ProtocolMetadata for Hysteria2Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::hysteria2::Hysteria2Protocol.descriptor()
    }
}
