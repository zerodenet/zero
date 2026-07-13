#[cfg(feature = "vmess")]
use async_trait::async_trait;
#[cfg(feature = "vmess")]
mod listener;
#[cfg(feature = "vmess")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "vmess")]
#[cfg(feature = "vmess")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
use zero_transport::vmess_transport::VmessStreamBridge;

use crate::adapters::identity::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter, ProtocolTransportBridgeAdapter,
};
use crate::protocol_registry::{
    proxy_leaf_runtime, InboundListenerCapability, ManagedUdpHandlerProvider, OutboundLeafRuntime,
    ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "vmess")]
use crate::runtime::tcp_dispatch::operation::{
    prepare_transport_bridge_tcp_connect, prepare_transport_bridge_tcp_relay,
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
#[cfg(feature = "vmess")]
use crate::runtime::udp_dispatch::operation::{
    PreparedTransportUdpOperation, PreparedUdpFlowOperation, TransportBridgeUdpOperation,
};
#[cfg(feature = "vmess")]
use crate::runtime::udp_dispatch::FlowFailure;
#[cfg(feature = "vmess")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_bridge, ManagedStreamHandlerPair,
};
#[cfg(feature = "vmess")]
use crate::transport::TcpOutboundFailure;

#[cfg(feature = "vmess")]
#[derive(Debug, Default)]
pub(crate) struct VmessAdapter {
    bridge: VmessStreamBridge,
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

#[cfg(feature = "vmess")]
#[async_trait]
impl TcpOutboundCapability for VmessAdapter {
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

#[cfg(feature = "vmess")]
#[async_trait]
impl UdpFlowCapability for VmessAdapter {
    fn prepare_udp_flow<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        Ok(Box::new(TransportBridgeUdpOperation {
            bridge: self.bridge(),
            operation: PreparedTransportUdpOperation::Direct { leaf },
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

#[cfg(feature = "vmess")]
impl ManagedUdpHandlerProvider for VmessAdapter {
    fn managed_stream_udp_handlers(&self) -> Option<ManagedStreamHandlerPair> {
        Some(managed_stream_udp_handler_for_bridge::<VmessStreamBridge>())
    }
}

#[cfg(feature = "vmess")]
impl UdpPacketPathCapability for VmessAdapter {}
