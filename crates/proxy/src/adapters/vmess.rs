#[cfg(feature = "vmess")]
use async_trait::async_trait;
#[cfg(feature = "vmess")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "vmess")]
use zero_core::Session;
#[cfg(feature = "vmess")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
#[cfg(feature = "vmess")]
use zero_transport::vmess_transport::VmessInboundListenerRequest;
use zero_transport::vmess_transport::VmessStreamBridge;

use crate::adapters::common::{
    apply_protocol_transport_bridge_adapter_relay_hop,
    connect_protocol_transport_bridge_adapter_tcp, named_protocol_supports_inbound,
    named_protocol_supports_outbound, start_protocol_transport_bridge_adapter_udp_flow,
    start_protocol_transport_bridge_adapter_udp_relay_final_hop,
    transport_bridge_adapter_claims_runtime_leaf, transport_bridge_adapter_leaf_runtime,
    transport_bridge_adapter_managed_stream_udp_handler, NamedProtocolAdapter,
    ProtocolTransportBridgeAdapter,
};
use crate::protocol_registry::{
    BoundInbound, InboundAdapterContext, InboundListenerCapability, OutboundAdapterContext,
    OutboundLeafRuntime, ProtocolSupportCapability, TcpOutboundCapability, UdpAdapterContext,
    UdpFlowCapability, UdpPacketPathCapability,
};
#[cfg(feature = "vmess")]
use crate::runtime::inbound_route::spawn_transport_mux_route_inbound_listener_with_request;
use crate::runtime::orchestration::TcpPathCategory;
#[cfg(feature = "vmess")]
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
#[cfg(feature = "vmess")]
use crate::runtime::udp_flow::managed::ManagedStreamFlowHandler;
#[cfg(feature = "vmess")]
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

#[cfg(feature = "vmess")]
#[derive(Debug)]
pub(crate) struct VmessAdapter {
    bridge: VmessStreamBridge,
}

#[cfg(feature = "vmess")]
impl Default for VmessAdapter {
    fn default() -> Self {
        Self {
            bridge: VmessStreamBridge::default(),
        }
    }
}

#[cfg(feature = "vmess")]
impl NamedProtocolAdapter for VmessAdapter {
    const PROTOCOL_NAME: &'static str = "vmess";
    const FEATURE_NAME: &'static str = "vmess";
}

#[cfg(feature = "vmess")]
impl ProtocolTransportBridgeAdapter for VmessAdapter {
    type Bridge = VmessStreamBridge;

    const TCP_PATH: TcpPathCategory = TcpPathCategory::Session;

    fn bridge(&self) -> &Self::Bridge {
        &self.bridge
    }
}

#[cfg(feature = "vmess")]
impl ProtocolSupportCapability for VmessAdapter {
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

#[cfg(feature = "vmess")]
impl ProtocolMetadata for VmessAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::vmess::metadata::VmessProtocol.descriptor()
    }
}

#[cfg(feature = "vmess")]
impl InboundListenerCapability for VmessAdapter {
    fn spawn_inbound(
        &self,
        ctx: InboundAdapterContext<'_>,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        spawn_transport_mux_route_inbound_listener_with_request::<VmessInboundListenerRequest>(
            ctx.proxy(),
            inbound,
            bound,
            shutdown_rx,
            listeners,
        );
    }
}

#[cfg(feature = "vmess")]
#[async_trait]
impl TcpOutboundCapability for VmessAdapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        transport_bridge_adapter_claims_runtime_leaf::<Self>(leaf)
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        transport_bridge_adapter_leaf_runtime::<Self>(leaf)
    }

    async fn connect_tcp(
        &self,
        ctx: OutboundAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        connect_protocol_transport_bridge_adapter_tcp(self, ctx, session, leaf, |_| {}).await
    }

    async fn apply_relay_hop(
        &self,
        ctx: OutboundAdapterContext<'_>,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        apply_protocol_transport_bridge_adapter_relay_hop(self, ctx, stream, session, leaf).await
    }
}

#[cfg(feature = "vmess")]
#[async_trait]
impl UdpFlowCapability for VmessAdapter {
    fn managed_stream_udp_handler(&self) -> Option<Box<dyn ManagedStreamFlowHandler>> {
        Some(transport_bridge_adapter_managed_stream_udp_handler::<Self>())
    }

    async fn start_udp_flow(
        &self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        start_protocol_transport_bridge_adapter_udp_flow(
            self, dispatch, ctx, session, leaf, payload,
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
        start_protocol_transport_bridge_adapter_udp_relay_final_hop(
            self, dispatch, ctx, session, carrier, leaf, payload,
        )
        .await
    }
}

#[cfg(feature = "vmess")]
impl UdpPacketPathCapability for VmessAdapter {}
