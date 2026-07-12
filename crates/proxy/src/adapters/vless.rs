#[cfg(feature = "vless")]
use async_trait::async_trait;
#[cfg(feature = "vless")]
mod listener;
#[cfg(feature = "vless")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "vless")]
use zero_core::Session;
#[cfg(feature = "vless")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
#[cfg(feature = "vless")]
use zero_transport::vless_transport::OwnedVlessInboundBindPlan;
use zero_transport::vless_transport::VlessStreamBridge;

use crate::adapters::common::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter, ProtocolTransportBridgeAdapter,
};
use crate::protocol_registry::{
    bind_transport_inbound, proxy_leaf_runtime, BoundInbound, InboundAdapterContext,
    InboundListenerCapability, OutboundAdapterContext, OutboundLeafRuntime,
    ProtocolSupportCapability, TcpOutboundCapability, UdpAdapterContext, UdpFlowCapability,
    UdpPacketPathCapability,
};
use crate::runtime::orchestration::TcpPathCategory;
#[cfg(feature = "vless")]
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
#[cfg(feature = "vless")]
use crate::runtime::udp_flow::managed::{
    bridge::{
        managed_stream_udp_handler_for_bridge,
        protocol_transport_bridge_udp_relay_needs_two_streams,
        start_protocol_transport_bridge_udp_flow,
        start_protocol_transport_bridge_udp_relay_final_hop,
        start_protocol_transport_bridge_udp_relay_two_stream,
    },
    ManagedStreamFlowHandler,
};
#[cfg(feature = "vless")]
use crate::transport::{
    apply_protocol_transport_bridge_relay_hop, connect_protocol_transport_bridge_tcp,
    EstablishedTcpOutbound, TcpOutboundFailure,
};

#[cfg(feature = "vless")]
#[derive(Debug, Default)]
pub(crate) struct VlessAdapter {
    bridge: VlessStreamBridge,
}

#[cfg(feature = "vless")]
impl NamedProtocolAdapter for VlessAdapter {
    const PROTOCOL_NAME: &'static str = "vless";
    const FEATURE_NAME: &'static str = "vless";
}

#[cfg(feature = "vless")]
impl ProtocolTransportBridgeAdapter for VlessAdapter {
    type Bridge = VlessStreamBridge;

    const TCP_PATH: TcpPathCategory = TcpPathCategory::Tunnel;

    fn bridge(&self) -> &Self::Bridge {
        &self.bridge
    }
}

#[cfg(feature = "vless")]
impl ProtocolSupportCapability for VlessAdapter {
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

    fn on_config_reloaded(&self) {
        self.bridge.on_config_reloaded();
    }
}

#[cfg(feature = "vless")]
impl ProtocolMetadata for VlessAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::vless::metadata::VlessProtocol.descriptor()
    }
}

#[cfg(feature = "vless")]
#[async_trait]
impl InboundListenerCapability for VlessAdapter {
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        bind_transport_inbound::<OwnedVlessInboundBindPlan>(inbound, source_dir).await
    }

    fn spawn_inbound(
        &self,
        ctx: InboundAdapterContext<'_>,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        listener::spawn(ctx.proxy(), inbound, bound, shutdown_rx, listeners);
    }
}

#[cfg(feature = "vless")]
#[async_trait]
impl TcpOutboundCapability for VlessAdapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        named_protocol_claims_runtime_leaf::<Self>(leaf)
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        proxy_leaf_runtime(leaf, Self::TCP_PATH)
    }

    async fn connect_tcp(
        &self,
        ctx: OutboundAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        connect_protocol_transport_bridge_tcp(self.bridge(), ctx, session, leaf, |_| {}).await
    }

    async fn apply_relay_hop(
        &self,
        ctx: OutboundAdapterContext<'_>,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        apply_protocol_transport_bridge_relay_hop(self.bridge(), ctx, stream, session, leaf).await
    }
}

#[cfg(feature = "vless")]
#[async_trait]
impl UdpFlowCapability for VlessAdapter {
    fn managed_stream_udp_handler(&self) -> Option<Box<dyn ManagedStreamFlowHandler>> {
        Some(managed_stream_udp_handler_for_bridge::<VlessStreamBridge>())
    }

    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        start_protocol_transport_bridge_udp_flow(
            self.bridge(),
            dispatch,
            ctx.proxy(),
            session,
            leaf,
            payload,
        )
        .await
    }

    fn udp_relay_needs_two_streams(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        protocol_transport_bridge_udp_relay_needs_two_streams(self.bridge(), leaf)
    }

    async fn start_udp_relay_two_stream(
        &self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        start_protocol_transport_bridge_udp_relay_two_stream(
            self.bridge(),
            dispatch,
            ctx.proxy(),
            session,
            &chain,
            payload,
        )
        .await
    }

    async fn start_udp_relay_final_hop(
        &self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        start_protocol_transport_bridge_udp_relay_final_hop(
            self.bridge(),
            dispatch,
            ctx.proxy(),
            session,
            carrier,
            leaf,
            payload,
        )
        .await
    }
}

#[cfg(feature = "vless")]
impl UdpPacketPathCapability for VlessAdapter {}
