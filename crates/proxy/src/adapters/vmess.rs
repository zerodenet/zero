use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::common::proxy_leaf_runtime;
use crate::protocol_registry::{
    BoundInbound, InboundAdapterContext, InboundListenerCapability, OutboundAdapterContext,
    OutboundLeafRuntime, ProtocolSupportCapability, TcpOutboundCapability, UdpAdapterContext,
    UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::orchestration::TcpPathCategory;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

#[cfg(feature = "vmess")]
mod inbound;
#[cfg(feature = "vmess")]
pub(crate) mod mux_pool;
#[cfg(feature = "vmess")]
mod tcp;
#[cfg(feature = "vmess")]
mod udp;

#[cfg(feature = "vmess")]
#[derive(Debug)]
pub(crate) struct VmessAdapter {
    mux_pool: vmess::mux::VmessMuxConnectionPool,
}

#[cfg(feature = "vmess")]
impl Default for VmessAdapter {
    fn default() -> Self {
        Self {
            mux_pool: vmess::mux::VmessMuxConnectionPool::new(),
        }
    }
}

#[cfg(feature = "vmess")]
#[async_trait]
impl UdpFlowCapability for VmessAdapter {
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

#[cfg(feature = "vmess")]
impl UdpPacketPathCapability for VmessAdapter {}

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
        self.spawn_inbound_impl(ctx.proxy(), inbound, bound, shutdown_rx, listeners);
    }
}

#[cfg(feature = "vmess")]
#[async_trait]
impl TcpOutboundCapability for VmessAdapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        matches!(leaf, ResolvedLeafOutbound::Vmess { .. })
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

#[cfg(feature = "vmess")]
impl ProtocolSupportCapability for VmessAdapter {
    fn name(&self) -> &'static str {
        "vmess"
    }
    fn feature_name(&self) -> &'static str {
        "vmess"
    }
    fn has_inbound(&self) -> bool {
        true
    }
    fn has_outbound(&self) -> bool {
        true
    }
    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Vmess { .. })
    }
    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        matches!(c, OutboundProtocolConfig::Vmess { .. })
    }

    fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }
}

#[cfg(feature = "vmess")]
impl ProtocolMetadata for VmessAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::vmess::VmessProtocol.descriptor()
    }
}
