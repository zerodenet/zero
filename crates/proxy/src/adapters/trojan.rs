#[cfg(feature = "trojan")]
use async_trait::async_trait;
#[cfg(feature = "trojan")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "trojan")]
use zero_core::Session;
#[cfg(feature = "trojan")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
#[cfg(feature = "trojan")]
use zero_transport::trojan_transport::TrojanInboundListenerRequest;
use zero_transport::trojan_transport::TrojanTlsBridge;

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
#[cfg(feature = "trojan")]
use crate::runtime::inbound_route::spawn_transport_stream_route_inbound_listener_with_request;
use crate::runtime::orchestration::TcpPathCategory;
#[cfg(feature = "trojan")]
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
#[cfg(feature = "trojan")]
use crate::runtime::udp_flow::managed::ManagedStreamFlowHandler;
#[cfg(feature = "trojan")]
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

#[cfg(feature = "trojan")]
#[derive(Debug)]
pub(crate) struct TrojanAdapter {
    bridge: TrojanTlsBridge,
}

#[cfg(feature = "trojan")]
impl Default for TrojanAdapter {
    fn default() -> Self {
        Self {
            bridge: TrojanTlsBridge,
        }
    }
}

#[cfg(feature = "trojan")]
impl NamedProtocolAdapter for TrojanAdapter {
    const PROTOCOL_NAME: &'static str = "trojan";
    const FEATURE_NAME: &'static str = "trojan";
}

#[cfg(feature = "trojan")]
impl ProtocolTransportBridgeAdapter for TrojanAdapter {
    type Bridge = TrojanTlsBridge;

    const TCP_PATH: TcpPathCategory = TcpPathCategory::Tunnel;

    fn bridge(&self) -> &Self::Bridge {
        &self.bridge
    }
}

#[cfg(feature = "trojan")]
impl ProtocolSupportCapability for TrojanAdapter {
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

#[cfg(feature = "trojan")]
impl ProtocolMetadata for TrojanAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::trojan::metadata::TrojanProtocol.descriptor()
    }
}

#[cfg(feature = "trojan")]
impl InboundListenerCapability for TrojanAdapter {
    fn spawn_inbound(
        &self,
        ctx: InboundAdapterContext<'_>,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        spawn_transport_stream_route_inbound_listener_with_request::<TrojanInboundListenerRequest>(
            ctx.proxy(),
            inbound,
            bound,
            shutdown_rx,
            listeners,
        );
    }
}

#[cfg(feature = "trojan")]
#[async_trait]
impl TcpOutboundCapability for TrojanAdapter {
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
        connect_protocol_transport_bridge_adapter_tcp(self, ctx, session, leaf, |traffic| {
            tracing::debug!(
                session_id = session.id,
                trojan_handshake_tx = traffic.written_bytes,
                target = ?session.target,
                target_port = session.port,
                "trojan upstream connected"
            );
        })
        .await
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

#[cfg(feature = "trojan")]
#[async_trait]
impl UdpFlowCapability for TrojanAdapter {
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

#[cfg(feature = "trojan")]
impl UdpPacketPathCapability for TrojanAdapter {}
