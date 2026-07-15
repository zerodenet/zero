#[cfg(feature = "vmess")]
use async_trait::async_trait;
#[cfg(feature = "vmess")]
mod listener;
use ::vmess::transport::{VmessOutboundLeaf, VmessStreamBridge};
#[cfg(feature = "vmess")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "vmess")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter, ProtocolTransportBridgeAdapter,
};
use crate::adapters::transport_bridge::{
    claim_transport_bridge_tcp_leaf, prepare_transport_bridge_leaf,
    transport_bridge_connect_prepare_failure, transport_bridge_relay_prepare_error,
    transport_bridge_udp_direct_prepare_failure, transport_bridge_udp_relay_final_prepare_failure,
    ProtocolTransportLeafResolver,
};
use crate::protocol_registry::{
    prepare_owned_transport_bridge_udp_relay_final_hop, prepare_transport_bridge_tcp_connect,
    prepare_transport_bridge_tcp_relay, prepare_transport_bridge_udp_direct, proxy_leaf_runtime,
    ClaimedTcpOutboundLeaf, InboundListenerCapability, ManagedUdpHandlerProvider,
    OutboundLeafRuntime, ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability,
    UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "vmess")]
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
#[cfg(feature = "vmess")]
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
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
impl ProtocolTransportLeafResolver for VmessStreamBridge {
    type TransportLeaf = VmessOutboundLeaf;
    type ResolveError = zero_core::Error;

    fn resolve_transport_leaf<'a>(
        &self,
        source_dir: Option<&std::path::Path>,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<Option<Self::TransportLeaf>, Self::ResolveError> {
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
            return Ok(None);
        };
        let resolved = VmessOutboundLeaf::from_profile_refs(
            source_dir,
            tag,
            server,
            *port,
            id,
            cipher,
            *mux_concurrency,
            *tls,
            *ws,
            *grpc,
        )?;
        Ok(Some(resolved))
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

    fn claim_tcp_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
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
            move |source_dir| {
                VmessOutboundLeaf::from_profile_refs(
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
            },
        ))
    }

    fn outbound_leaf_runtime(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<OutboundLeafRuntime> {
        proxy_leaf_runtime(leaf, Self::TCP_PATH)
    }

    fn prepare_tcp_connect<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        let prepared =
            prepare_transport_bridge_leaf(self.bridge(), source_dir, &leaf).map_err(|error| {
                transport_bridge_connect_prepare_failure::<VmessStreamBridge, _>(&leaf, error)
            })?;
        Ok(prepare_transport_bridge_tcp_connect(
            self.bridge(),
            prepared,
        ))
    }

    fn prepare_tcp_relay_hop<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        let prepared = prepare_transport_bridge_leaf(self.bridge(), source_dir, &leaf)
            .map_err(transport_bridge_relay_prepare_error::<VmessStreamBridge, _>)?;
        Ok(prepare_transport_bridge_tcp_relay(self.bridge(), prepared))
    }
}

#[cfg(feature = "vmess")]
#[async_trait]
impl UdpFlowCapability for VmessAdapter {
    fn prepare_udp_flow<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared =
            prepare_transport_bridge_leaf(self.bridge(), source_dir, &leaf).map_err(|error| {
                transport_bridge_udp_direct_prepare_failure::<VmessStreamBridge, _>(&leaf, error)
            })?;
        Ok(prepare_transport_bridge_udp_direct(self.bridge(), prepared))
    }

    fn prepare_owned_udp_relay_final_hop<'a>(
        &self,
        carrier: crate::transport::RelayCarrier,
        leaf: ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared =
            prepare_transport_bridge_leaf(self.bridge(), source_dir, &leaf).map_err(|error| {
                transport_bridge_udp_relay_final_prepare_failure::<VmessStreamBridge, _>(
                    &leaf, error,
                )
            })?;
        Ok(prepare_owned_transport_bridge_udp_relay_final_hop(
            self.bridge(),
            carrier,
            prepared,
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
