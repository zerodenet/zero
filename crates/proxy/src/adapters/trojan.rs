#[cfg(feature = "trojan")]
use async_trait::async_trait;
#[cfg(feature = "trojan")]
mod listener;
#[cfg(feature = "trojan")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "trojan")]
#[cfg(feature = "trojan")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
use zero_transport::trojan_transport::TrojanTlsBridge;

use crate::adapters::identity::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter, ProtocolTransportBridgeAdapter,
};
use crate::protocol_registry::{
    proxy_leaf_runtime, InboundListenerCapability, ManagedUdpHandlerProvider, OutboundLeafRuntime,
    ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "trojan")]
use crate::runtime::tcp_dispatch::operation::{
    prepare_transport_bridge_tcp_connect, prepare_transport_bridge_tcp_relay,
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
#[cfg(feature = "trojan")]
use crate::runtime::udp_dispatch::operation::{
    PreparedTransportUdpOperation, PreparedUdpFlowOperation, TransportBridgeUdpOperation,
};
#[cfg(feature = "trojan")]
use crate::runtime::udp_dispatch::FlowFailure;
#[cfg(feature = "trojan")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_bridge, ManagedStreamHandlerPair,
};
#[cfg(feature = "trojan")]
use crate::transport::TcpOutboundFailure;

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

#[cfg(feature = "trojan")]
#[async_trait]
impl TcpOutboundCapability for TrojanAdapter {
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

#[cfg(feature = "trojan")]
#[async_trait]
impl UdpFlowCapability for TrojanAdapter {
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

#[cfg(feature = "trojan")]
impl ManagedUdpHandlerProvider for TrojanAdapter {
    fn managed_stream_udp_handlers(&self) -> Option<ManagedStreamHandlerPair> {
        Some(managed_stream_udp_handler_for_bridge::<TrojanTlsBridge>())
    }
}

#[cfg(feature = "trojan")]
impl UdpPacketPathCapability for TrojanAdapter {}
