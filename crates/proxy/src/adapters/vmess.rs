#[cfg(feature = "vmess")]
mod listener;
use ::vmess::transport::{OwnedVmessOutboundLeafConfig, VmessOutboundLeaf, VmessStreamBridge};
#[cfg(feature = "vmess")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "vmess")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::{
    named_protocol_supports_inbound, named_protocol_supports_outbound, NamedProtocolAdapter,
    ProtocolTransportBridgeAdapter,
};
use crate::adapters::transport_bridge::{
    claim_transport_bridge_tcp_leaf, claim_transport_bridge_udp_leaf,
};
use crate::protocol_registry::{
    proxy_leaf_runtime, ClaimedTcpOutboundLeaf, ClaimedUdpFlowLeaf, InboundListenerCapability,
    ManagedUdpHandlerProvider, ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability,
    UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "vmess")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_bridge, ManagedStreamHandlerPair,
};

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
impl TcpOutboundCapability for VmessAdapter {
    fn claim_tcp_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        let runtime = proxy_leaf_runtime(&leaf, Self::TCP_PATH)?;
        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            tls,
            ws,
            grpc,
            ..
        } = leaf
        else {
            return None;
        };
        let bridge = self.bridge.clone();
        Some(claim_transport_bridge_tcp_leaf(
            bridge,
            Some((server, port)),
            runtime,
            move |source_dir| {
                OwnedVmessOutboundLeafConfig::from_config_refs(
                    source_dir,
                    tag,
                    server,
                    port,
                    id,
                    cipher,
                    mux_concurrency,
                    tls,
                    ws,
                    grpc,
                )
                .map(VmessOutboundLeaf::from)
            },
        ))
    }
}

#[cfg(feature = "vmess")]
impl UdpFlowCapability for VmessAdapter {
    fn claim_udp_flow_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
        let ResolvedLeafOutbound::Vmess {
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            tls,
            ws,
            grpc,
            ..
        } = leaf
        else {
            return None;
        };
        let bridge = self.bridge.clone();
        Some(claim_transport_bridge_udp_leaf(
            bridge,
            Some((server, port)),
            move |source_dir| {
                OwnedVmessOutboundLeafConfig::from_config_refs(
                    source_dir,
                    tag,
                    server,
                    port,
                    id,
                    cipher,
                    mux_concurrency,
                    tls,
                    ws,
                    grpc,
                )
                .map(VmessOutboundLeaf::from)
            },
        ))
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
