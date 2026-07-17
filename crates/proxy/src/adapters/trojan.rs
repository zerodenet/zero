#[cfg(feature = "trojan")]
mod listener;
use ::trojan::transport::{
    TrojanInboundListenerRequest, TrojanInboundOptionsRef, TrojanOutboundBuildOptionsRef,
    TrojanOutboundLeaf, TrojanOutboundOptionsRef,
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
    claim_transport_tcp_leaf, claim_transport_udp_leaf, InboundListenerCapability,
    ManagedUdpHandlerProvider, OutboundLeafClaim, OutboundLeafRuntime, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "trojan")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_resume, ManagedStreamHandlerPair,
};

#[cfg(feature = "trojan")]
#[derive(Debug, Default)]
pub(crate) struct TrojanAdapter;

#[cfg(feature = "trojan")]
#[derive(Clone, Copy)]
struct TrojanOutboundProjection<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    password: &'a str,
    sni: Option<&'a str>,
    insecure: bool,
    client_fingerprint: Option<&'a str>,
}

#[cfg(feature = "trojan")]
impl<'a> TrojanOutboundProjection<'a> {
    fn from_config(tag: &'a str, protocol: &'a OutboundProtocolConfig) -> Option<Self> {
        let OutboundProtocolConfig::Trojan {
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } = protocol
        else {
            return None;
        };
        Some(Self {
            tag,
            server,
            port: *port,
            password,
            sni: sni.as_deref(),
            insecure: *insecure,
            client_fingerprint: client_fingerprint.as_deref(),
        })
    }

    fn endpoint(&self) -> (&'a str, u16) {
        (self.server, self.port)
    }

    fn build_options(&self) -> TrojanOutboundBuildOptionsRef<'a> {
        TrojanOutboundBuildOptionsRef {
            tag: self.tag,
            server: self.server,
            port: self.port,
            protocol: TrojanOutboundOptionsRef {
                password: self.password,
                sni: self.sni,
                insecure: self.insecure,
                client_fingerprint: self.client_fingerprint,
            },
        }
    }
}

#[cfg(feature = "trojan")]
const TCP_PATH: TcpPathCategory = TcpPathCategory::Tunnel;

#[cfg(feature = "trojan")]
impl TrojanAdapter {
    pub(crate) fn claim_outbound_leaf_impl<'a>(
        &self,
        protocol: Option<&'a OutboundProtocolConfig>,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafClaim<'a>> {
        let projection = TrojanOutboundProjection::from_config(leaf.tag()?, protocol?)?;
        let runtime = OutboundLeafRuntime::proxy(
            projection.tag,
            projection.server,
            projection.port,
            TCP_PATH,
        );
        let endpoint = Some(projection.endpoint());
        Some(OutboundLeafClaim {
            runtime: runtime.clone(),
            tcp: claim_transport_tcp_leaf(endpoint, move |source_dir| {
                Ok::<TrojanOutboundLeaf, zero_core::Error>(TrojanOutboundLeaf::from_options_refs(
                    source_dir,
                    projection.build_options(),
                ))
            }),
            udp: Some(claim_transport_udp_leaf(endpoint, move |source_dir| {
                Ok::<TrojanOutboundLeaf, zero_core::Error>(TrojanOutboundLeaf::from_options_refs(
                    source_dir,
                    projection.build_options(),
                ))
            })),
            packet_path: None,
        })
    }
}

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
        let request = match &inbound.protocol {
            InboundProtocolConfig::Trojan { password, tls, .. } => {
                TrojanInboundListenerRequest::from_options_refs(
                    source_dir,
                    TrojanInboundOptionsRef { password },
                    tls.as_ref(),
                )
                .map_err(EngineError::from)?
            }
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "trojan inbound listener received non-trojan inbound config",
                )));
            }
        };
        Ok(listener::prepare(request))
    }
}

#[cfg(feature = "trojan")]
impl TcpOutboundCapability for TrojanAdapter {}

#[cfg(feature = "trojan")]
impl UdpFlowCapability for TrojanAdapter {}

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
