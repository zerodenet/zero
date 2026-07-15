#[cfg(feature = "vless")]
use ::vless::transport::{
    VlessInboundBindPlan, VlessOutboundLeaf, VlessQuicBindProfile, VlessQuicClientProfile,
    VlessRealityClientProfile, VlessStreamBridge,
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
    named_protocol_supports_inbound, named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::adapters::transport_bridge::{
    claim_relay_two_stream_transport_bridge_udp_leaf, claim_transport_bridge_tcp_leaf,
};
use crate::protocol_registry::{
    bind_transport_inbound, proxy_leaf_runtime, BoundInbound, ClaimedTcpOutboundLeaf,
    ClaimedUdpFlowLeaf, InboundListenerCapability, ManagedUdpHandlerProvider,
    ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "vless")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_bridge, ManagedStreamHandlerPair,
};

#[cfg(feature = "vless")]
#[derive(Debug, Default)]
pub(crate) struct VlessAdapter {
    bridge: VlessStreamBridge,
}

#[cfg(feature = "vless")]
const TCP_PATH: TcpPathCategory = TcpPathCategory::Tunnel;

#[cfg(feature = "vless")]
fn outbound_reality_profile(reality: Option<&RealityConfig>) -> Option<VlessRealityClientProfile> {
    reality.map(|reality| {
        VlessRealityClientProfile::new(
            reality.public_key.clone(),
            reality.short_id.clone(),
            reality.server_name.clone(),
            reality.cipher_suites.clone(),
        )
    })
}

#[cfg(feature = "vless")]
fn quic_client_profile(quic: Option<&QuicConfig>) -> Option<VlessQuicClientProfile> {
    quic.map(|quic| {
        VlessQuicClientProfile::new(
            quic.server_name.clone(),
            quic.insecure,
            quic.ca_cert_path.clone(),
        )
    })
}

#[cfg(feature = "vless")]
fn quic_bind_profile(quic: Option<&QuicConfig>) -> Option<VlessQuicBindProfile> {
    quic.map(|quic| VlessQuicBindProfile::new(quic.cert_path.clone(), quic.key_path.clone()))
}

#[cfg(feature = "vless")]
impl NamedProtocolAdapter for VlessAdapter {
    const PROTOCOL_NAME: &'static str = "vless";
    const FEATURE_NAME: &'static str = "vless";
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
        let plan = VlessInboundBindPlan::from_quic_profile(source_dir, quic.as_ref());
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
impl TcpOutboundCapability for VlessAdapter {
    fn claim_tcp_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        let runtime = proxy_leaf_runtime(&leaf, TCP_PATH)?;
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
            return None;
        };
        let bridge = self.bridge.clone();
        let reality = outbound_reality_profile(reality);
        let quic = quic_client_profile(quic);
        Some(claim_transport_bridge_tcp_leaf(
            bridge,
            Some((server, port)),
            runtime,
            move |source_dir| {
                VlessOutboundLeaf::from_config_refs(
                    source_dir,
                    tag,
                    server,
                    port,
                    id,
                    flow,
                    mux_concurrency,
                    tls,
                    reality.as_ref(),
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    quic.as_ref(),
                )
            },
        ))
    }
}

#[cfg(feature = "vless")]
impl UdpFlowCapability for VlessAdapter {
    fn claim_udp_flow_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
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
            return None;
        };
        let bridge = self.bridge.clone();
        let reality = outbound_reality_profile(reality);
        let quic = quic_client_profile(quic);
        Some(claim_relay_two_stream_transport_bridge_udp_leaf(
            bridge,
            Some((server, port)),
            move |source_dir| {
                VlessOutboundLeaf::from_config_refs(
                    source_dir,
                    tag,
                    server,
                    port,
                    id,
                    flow,
                    mux_concurrency,
                    tls,
                    reality.as_ref(),
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    quic.as_ref(),
                )
            },
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
