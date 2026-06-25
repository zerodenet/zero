use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::common::proxy_leaf_runtime;
use crate::protocol_adapter::{
    BoundInbound, InboundAdapterContext, OutboundAdapterContext, OutboundLeafRuntime,
    ProtocolAdapter, ProtocolSupportCapability, UdpAdapterContext,
};
use crate::runtime::orchestration::TcpPathCategory;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

#[cfg(feature = "trojan")]
mod inbound;
#[cfg(feature = "trojan")]
mod tcp;
#[cfg(feature = "trojan")]
mod udp;

#[cfg(feature = "trojan")]
#[derive(Debug)]
pub(crate) struct TrojanAdapter;

#[cfg(feature = "trojan")]
#[async_trait]
impl ProtocolAdapter for TrojanAdapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Trojan { .. })
    }
    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        proxy_leaf_runtime(leaf, TcpPathCategory::Tunnel)
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
        ctx: OutboundAdapterContext<'_>,
        stream: crate::transport::TcpRelayStream,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<crate::transport::TcpRelayStream, EngineError> {
        self.apply_relay_hop_impl(ctx.proxy(), stream, session, leaf)
            .await
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
    async fn start_udp_relay_final_hop(
        &self,
        dispatch: &mut UdpDispatch,
        ctx: UdpAdapterContext<'_>,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        self.start_udp_relay_final_hop_impl(dispatch, ctx.proxy(), session, carrier, leaf, payload)
            .await
    }
}

#[cfg(feature = "trojan")]
impl ProtocolSupportCapability for TrojanAdapter {
    fn name(&self) -> &'static str {
        "trojan"
    }
    fn feature_name(&self) -> &'static str {
        "trojan"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Trojan { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Trojan { .. })
    }
}

#[cfg(feature = "trojan")]
impl ProtocolMetadata for TrojanAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::trojan::TrojanProtocol.descriptor()
    }
}
