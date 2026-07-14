#[cfg(feature = "vless")]
use ::vless::transport::{
    OwnedVlessInboundBindPlan, OwnedVlessQuicBindProfile, OwnedVlessQuicClientProfile,
    OwnedVlessRealityClientProfile, VlessOutboundLeaf, VlessStreamBridge,
};
#[cfg(feature = "vless")]
use async_trait::async_trait;
#[cfg(feature = "vless")]
mod listener;
#[cfg(feature = "vless")]
use zero_config::{InboundConfig, QuicConfig, RealityConfig};
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "vless")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter, ProtocolTransportBridgeAdapter,
};
use crate::adapters::transport_bridge::{
    prepare_transport_bridge_leaf, transport_bridge_connect_prepare_failure,
    transport_bridge_relay_prepare_error, transport_bridge_udp_direct_prepare_failure,
    transport_bridge_udp_relay_final_prepare_failure,
    transport_bridge_udp_two_stream_prepare_failure, ProtocolTransportLeafResolver,
};
use crate::protocol_registry::{
    bind_transport_inbound, prepare_owned_transport_bridge_udp_relay_final_hop,
    prepare_owned_transport_bridge_udp_relay_two_stream, prepare_transport_bridge_tcp_connect,
    prepare_transport_bridge_tcp_relay, prepare_transport_bridge_udp_direct, proxy_leaf_runtime,
    transport_bridge_udp_relay_needs_two_streams, BoundInbound, InboundListenerCapability,
    ManagedUdpHandlerProvider, OutboundLeafRuntime, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "vless")]
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
#[cfg(feature = "vless")]
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
#[cfg(feature = "vless")]
use crate::runtime::udp_dispatch::FlowFailure;
#[cfg(feature = "vless")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_bridge, ManagedStreamHandlerPair,
};
#[cfg(feature = "vless")]
use crate::transport::TcpOutboundFailure;

#[cfg(feature = "vless")]
#[derive(Debug, Default)]
pub(crate) struct VlessAdapter {
    bridge: VlessStreamBridge,
}

#[cfg(feature = "vless")]
fn outbound_reality_profile(
    reality: Option<&RealityConfig>,
) -> Option<OwnedVlessRealityClientProfile> {
    reality.map(|reality| {
        OwnedVlessRealityClientProfile::new(
            reality.public_key.clone(),
            reality.short_id.clone(),
            reality.server_name.clone(),
            reality.cipher_suites.clone(),
        )
    })
}

#[cfg(feature = "vless")]
fn quic_client_profile(quic: Option<&QuicConfig>) -> Option<OwnedVlessQuicClientProfile> {
    quic.map(|quic| {
        OwnedVlessQuicClientProfile::new(
            quic.server_name.clone(),
            quic.insecure,
            quic.ca_cert_path.clone(),
        )
    })
}

#[cfg(feature = "vless")]
fn quic_bind_profile(quic: Option<&QuicConfig>) -> Option<OwnedVlessQuicBindProfile> {
    quic.map(|quic| OwnedVlessQuicBindProfile::new(quic.cert_path.clone(), quic.key_path.clone()))
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
impl ProtocolTransportLeafResolver for VlessStreamBridge {
    type TransportLeaf = VlessOutboundLeaf;
    type ResolveError = zero_core::Error;

    fn resolve_transport_leaf<'a>(
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
        let reality = outbound_reality_profile(*reality);
        let quic = quic_client_profile(*quic);
        let resolved = VlessOutboundLeaf::from_profile_refs(
            source_dir,
            tag,
            server,
            *port,
            id,
            *flow,
            *mux_concurrency,
            *tls,
            reality.as_ref(),
            *ws,
            *grpc,
            *h2,
            *http_upgrade,
            *split_http,
            quic.as_ref(),
        )?;
        Ok(Some(resolved))
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
        let InboundProtocolConfig::Vless { quic, .. } = &inbound.protocol else {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vless inbound bind received non-vless inbound config",
            )));
        };
        let quic = quic_bind_profile(quic.as_deref());
        let plan = OwnedVlessInboundBindPlan::from_quic_profile(source_dir, quic.as_ref());
        bind_transport_inbound(inbound, plan).await
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
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        let prepared =
            prepare_transport_bridge_leaf(self.bridge(), source_dir, leaf).map_err(|error| {
                transport_bridge_connect_prepare_failure::<VlessStreamBridge, _>(leaf, error)
            })?;
        Ok(prepare_transport_bridge_tcp_connect(
            self.bridge(),
            prepared,
        ))
    }

    fn prepare_tcp_relay_hop<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        let prepared = prepare_transport_bridge_leaf(self.bridge(), source_dir, leaf)
            .map_err(transport_bridge_relay_prepare_error::<VlessStreamBridge, _>)?;
        Ok(prepare_transport_bridge_tcp_relay(self.bridge(), prepared))
    }
}

#[cfg(feature = "vless")]
#[async_trait]
impl UdpFlowCapability for VlessAdapter {
    fn prepare_udp_flow<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared =
            prepare_transport_bridge_leaf(self.bridge(), source_dir, leaf).map_err(|error| {
                transport_bridge_udp_direct_prepare_failure::<VlessStreamBridge, _>(leaf, error)
            })?;
        Ok(prepare_transport_bridge_udp_direct(self.bridge(), prepared))
    }

    fn udp_relay_needs_two_streams(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
        source_dir: Option<&std::path::Path>,
    ) -> bool {
        prepare_transport_bridge_leaf(self.bridge(), source_dir, leaf).is_ok_and(|prepared| {
            transport_bridge_udp_relay_needs_two_streams(self.bridge(), &prepared)
        })
    }

    fn prepare_owned_udp_relay_final_hop<'a>(
        &self,
        carrier: crate::transport::RelayCarrier,
        leaf: ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared =
            prepare_transport_bridge_leaf(self.bridge(), source_dir, &leaf).map_err(|error| {
                transport_bridge_udp_relay_final_prepare_failure::<VlessStreamBridge, _>(
                    &leaf, error,
                )
            })?;
        Ok(prepare_owned_transport_bridge_udp_relay_final_hop(
            self.bridge(),
            carrier,
            prepared,
        ))
    }

    fn prepare_owned_udp_relay_two_stream<'a>(
        &self,
        post_carrier: crate::transport::RelayCarrier,
        get_carrier: crate::transport::RelayCarrier,
        leaf: ResolvedLeafOutbound<'a>,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let prepared =
            prepare_transport_bridge_leaf(self.bridge(), source_dir, &leaf).map_err(|error| {
                transport_bridge_udp_two_stream_prepare_failure::<VlessStreamBridge, _>(
                    &leaf, error,
                )
            })?;
        Ok(prepare_owned_transport_bridge_udp_relay_two_stream(
            self.bridge(),
            post_carrier,
            get_carrier,
            prepared,
        ))
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
