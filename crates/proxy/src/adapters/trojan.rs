#[cfg(feature = "trojan")]
mod listener;
use ::trojan::transport::{
    TrojanOutboundBuildOptionsRef, TrojanOutboundLeaf, TrojanOutboundOptionsRef,
    TrojanTransportRuntime,
};
#[cfg(feature = "trojan")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "trojan")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
#[cfg(feature = "trojan")]
use zero_transport::managed_udp::ProtocolManagedStreamUdpLeafOps;

use crate::adapters::identity::{
    named_protocol_supports_inbound, named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    claim_transport_tcp_leaf, claim_transport_udp_leaf, proxy_leaf_runtime, ClaimedTcpOutboundLeaf,
    ClaimedUdpFlowLeaf, InboundListenerCapability, ManagedUdpHandlerProvider,
    ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "trojan")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_resume, ManagedStreamHandlerPair,
};

#[cfg(feature = "trojan")]
#[derive(Debug, Default)]
pub(crate) struct TrojanAdapter {
    runtime: TrojanTransportRuntime,
}

#[cfg(feature = "trojan")]
const TCP_PATH: TcpPathCategory = TcpPathCategory::Tunnel;

#[cfg(feature = "trojan")]
impl NamedProtocolAdapter for TrojanAdapter {
    const PROTOCOL_NAME: &'static str = "trojan";
    const FEATURE_NAME: &'static str = "trojan";
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
        listener::prepare(self.runtime.clone(), inbound, source_dir)
    }
}

#[cfg(feature = "trojan")]
impl TcpOutboundCapability for TrojanAdapter {
    fn claim_tcp_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        let runtime = proxy_leaf_runtime(&leaf, TCP_PATH)?;
        let ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } = leaf
        else {
            return None;
        };
        let transport_runtime = self.runtime.clone();
        Some(claim_transport_tcp_leaf(
            Some((server, port)),
            runtime,
            move |source_dir| {
                transport_runtime.build_outbound_leaf(
                    source_dir,
                    TrojanOutboundBuildOptionsRef {
                        tag,
                        server,
                        port,
                        protocol: TrojanOutboundOptionsRef {
                            password,
                            sni,
                            insecure,
                            client_fingerprint,
                        },
                    },
                )
            },
        ))
    }
}

#[cfg(feature = "trojan")]
impl UdpFlowCapability for TrojanAdapter {
    fn claim_udp_flow_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
        let ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } = leaf
        else {
            return None;
        };
        let transport_runtime = self.runtime.clone();
        Some(claim_transport_udp_leaf(
            Some((server, port)),
            move |source_dir| {
                transport_runtime.build_outbound_leaf(
                    source_dir,
                    TrojanOutboundBuildOptionsRef {
                        tag,
                        server,
                        port,
                        protocol: TrojanOutboundOptionsRef {
                            password,
                            sni,
                            insecure,
                            client_fingerprint,
                        },
                    },
                )
            },
        ))
    }
}

#[cfg(feature = "trojan")]
impl ManagedUdpHandlerProvider for TrojanAdapter {
    fn managed_stream_udp_handlers(&self) -> Option<ManagedStreamHandlerPair> {
        Some(managed_stream_udp_handler_for_resume::<
            <TrojanOutboundLeaf as ProtocolManagedStreamUdpLeafOps>::Resume,
        >())
    }
}

#[cfg(feature = "trojan")]
impl UdpPacketPathCapability for TrojanAdapter {}
