#[cfg(feature = "vless")]
use async_trait::async_trait;
#[cfg(feature = "vless")]
mod listener;
#[cfg(feature = "vless")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "vless")]
#[cfg(feature = "vless")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
#[cfg(feature = "vless")]
use zero_transport::vless_transport::OwnedVlessInboundBindPlan;
use zero_transport::vless_transport::{
    OwnedVlessOutboundTransportPlan, VlessOutboundLeaf, VlessStreamBridge,
};

use crate::adapters::identity::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter, ProtocolTransportBridgeAdapter,
};
use crate::protocol_registry::ProtocolTransportLeafResolver;
use crate::protocol_registry::{
    bind_transport_inbound, proxy_leaf_runtime, BoundInbound, InboundListenerCapability,
    ManagedUdpHandlerProvider, OutboundLeafRuntime, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "vless")]
use crate::runtime::tcp_dispatch::operation::{
    prepare_transport_bridge_tcp_connect, prepare_transport_bridge_tcp_relay,
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
#[cfg(feature = "vless")]
use crate::runtime::udp_dispatch::operation::{
    PreparedTransportUdpOperation, PreparedUdpFlowOperation, RelayTwoStreamUdpOperation,
    TransportBridgeUdpOperation,
};
#[cfg(feature = "vless")]
use crate::runtime::udp_dispatch::FlowFailure;
#[cfg(feature = "vless")]
use crate::runtime::udp_flow::managed::{
    bridge::{
        managed_stream_udp_handler_for_bridge,
        protocol_transport_bridge_udp_relay_needs_two_streams,
    },
    ManagedStreamHandlerPair,
};
#[cfg(feature = "vless")]
use crate::transport::TcpOutboundFailure;

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
impl<'a> ProtocolTransportLeafResolver<'a> for VlessStreamBridge {
    type TransportLeaf = VlessOutboundLeaf<'a>;
    type ResolveError = zero_core::Error;

    fn resolve_transport_leaf(
        &self,
        source_dir: Option<&std::path::Path>,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<Option<Self::TransportLeaf>, Self::ResolveError> {
        let ResolvedLeafOutbound::Vless {
            tag,
            server,
            port,
            id,
            flow,
            mux_concurrency,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            quic,
            ..
        } = leaf
        else {
            return Ok(None);
        };
        let transport = OwnedVlessOutboundTransportPlan::from_config_refs(
            source_dir,
            server,
            *port,
            *tls,
            *reality,
            *ws,
            *grpc,
            *h2,
            *http_upgrade,
            *split_http,
            *quic,
        );
        let protocol = ::vless::outbound::PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(
            id,
            *flow,
            *mux_concurrency,
            transport.mux_transport_hints(),
        )?;
        Ok(Some(VlessOutboundLeaf::new(
            tag, server, *port, transport, protocol,
        )))
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

    fn prepare_inbound_listener(
        &self,
        inbound: InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        listener::prepare(inbound, source_dir)
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

    fn prepare_tcp_connect<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        prepare_transport_bridge_tcp_connect(self.bridge(), source_dir, leaf)
    }

    fn prepare_tcp_relay_hop<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        prepare_transport_bridge_tcp_relay(self.bridge(), source_dir, leaf)
    }
}

#[cfg(feature = "vless")]
#[async_trait]
impl UdpFlowCapability for VlessAdapter {
    fn prepare_udp_flow<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        Ok(Box::new(TransportBridgeUdpOperation {
            bridge: self.bridge(),
            operation: PreparedTransportUdpOperation::Direct { leaf },
        }))
    }

    fn udp_relay_needs_two_streams(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        protocol_transport_bridge_udp_relay_needs_two_streams(self.bridge(), leaf)
    }

    fn prepare_udp_relay_two_stream<'a>(
        &'a self,
        chain: Vec<ResolvedLeafOutbound<'a>>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        Ok(Box::new(RelayTwoStreamUdpOperation {
            bridge: self.bridge(),
            chain,
        }))
    }

    fn prepare_udp_relay_final_hop<'a>(
        &'a self,
        carrier: crate::transport::RelayCarrier,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        Ok(Box::new(TransportBridgeUdpOperation {
            bridge: self.bridge(),
            operation: PreparedTransportUdpOperation::RelayFinalHop { carrier, leaf },
        }))
    }
}

#[cfg(feature = "vless")]
impl ManagedUdpHandlerProvider for VlessAdapter {
    fn managed_stream_udp_handlers(&self) -> Option<ManagedStreamHandlerPair> {
        Some(managed_stream_udp_handler_for_bridge::<VlessStreamBridge>())
    }
}

#[cfg(feature = "vless")]
impl UdpPacketPathCapability for VlessAdapter {}
