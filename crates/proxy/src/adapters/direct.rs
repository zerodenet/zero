use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::common::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_capability::protocol_descriptor;
use crate::protocol_registry::{
    direct_leaf_runtime, BoundInbound, InboundAdapterContext, InboundListenerCapability,
    OutboundAdapterContext, OutboundLeafRuntime, ProtocolSupportCapability, TcpOutboundCapability,
    UdpAdapterContext, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

mod inbound;
mod tcp;
mod udp;

// Direct inbound is always available (no feature gate).
#[derive(Debug)]
pub(crate) struct DirectAdapter;

impl NamedProtocolAdapter for DirectAdapter {
    const PROTOCOL_NAME: &'static str = "direct";
    const FEATURE_NAME: &'static str = "core";
    const HAS_OUTBOUND: bool = false;
}

#[async_trait]
impl UdpFlowCapability for DirectAdapter {
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

impl UdpPacketPathCapability for DirectAdapter {}

impl InboundListenerCapability for DirectAdapter {
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

#[async_trait]
impl TcpOutboundCapability for DirectAdapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        named_protocol_claims_runtime_leaf::<Self>(leaf)
    }
    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        direct_leaf_runtime(leaf)
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

impl ProtocolSupportCapability for DirectAdapter {
    fn name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::PROTOCOL_NAME
    }
    fn feature_name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::FEATURE_NAME
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        named_protocol_supports_inbound::<Self>(c)
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        named_protocol_supports_outbound::<Self>(c)
    }
    fn has_inbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_INBOUND
    }
    fn has_outbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_OUTBOUND
    }
}

impl ProtocolMetadata for DirectAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        protocol_descriptor("direct", "core")
    }
}
