use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
use zero_transport::hysteria2_quic::OwnedHysteria2InboundBindPlan;

use crate::adapters::identity::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    bind_transport_inbound, proxy_leaf_runtime, BoundInbound, InboundListenerCapability,
    ManagedUdpHandlerProvider, OutboundLeafRuntime, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;
use crate::transport::TcpOutboundFailure;

#[cfg(feature = "hysteria2")]
mod inbound;
#[cfg(feature = "hysteria2")]
mod tcp;
#[cfg(feature = "hysteria2")]
pub(crate) mod udp;

#[cfg(feature = "hysteria2")]
#[derive(Debug)]
pub(crate) struct Hysteria2Adapter;

#[cfg(feature = "hysteria2")]
impl NamedProtocolAdapter for Hysteria2Adapter {
    const PROTOCOL_NAME: &'static str = "hysteria2";
    const FEATURE_NAME: &'static str = "hysteria2";
}

#[cfg(feature = "hysteria2")]
impl UdpPacketPathCapability for Hysteria2Adapter {
    fn prepare_udp_packet_path<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Option<
        Box<
            dyn crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation
                + 'a,
        >,
    > {
        self.prepare_udp_packet_path_impl(leaf)
    }
}

#[cfg(feature = "hysteria2")]
#[async_trait]
impl UdpFlowCapability for Hysteria2Adapter {
    fn prepare_udp_flow<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<
        Box<dyn crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation + 'a>,
        FlowFailure,
    > {
        self.prepare_udp_flow_impl(leaf)
    }
}

#[cfg(feature = "hysteria2")]
impl ManagedUdpHandlerProvider for Hysteria2Adapter {
    fn managed_datagram_udp_handler(&self) -> Option<Box<dyn ManagedDatagramFlowHandler>> {
        Some(udp::managed_datagram_handler())
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
        bind_transport_inbound::<OwnedHysteria2InboundBindPlan>(inbound, source_dir).await
    }

    fn prepare_inbound_listener(
        &self,
        inbound: InboundConfig,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        self.prepare_inbound_listener_impl(inbound)
    }
}

#[cfg(feature = "hysteria2")]
#[async_trait]
impl TcpOutboundCapability for Hysteria2Adapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        named_protocol_claims_runtime_leaf::<Self>(leaf)
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        proxy_leaf_runtime(leaf, TcpPathCategory::TransportSession)
    }

    fn prepare_tcp_connect<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<
        Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpConnectOperation + 'a>,
        TcpOutboundFailure,
    > {
        self.prepare_tcp_connect_impl(leaf)
    }
}

#[cfg(feature = "hysteria2")]
impl ProtocolSupportCapability for Hysteria2Adapter {
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

#[cfg(feature = "hysteria2")]
impl ProtocolMetadata for Hysteria2Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::hysteria2::Hysteria2Protocol.descriptor()
    }
}
