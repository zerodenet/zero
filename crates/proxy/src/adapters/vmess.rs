#[cfg(feature = "vmess")]
mod listener;
use ::vmess::transport::{
    VmessInboundListenerRequest, VmessInboundOptionsRef, VmessInboundUserRef,
    VmessOutboundBuildOptionsRef, VmessOutboundLeaf, VmessOutboundOptionsRef,
    VmessTransportRuntime,
};
#[cfg(feature = "vmess")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "vmess")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
#[cfg(feature = "vmess")]
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
#[cfg(feature = "vmess")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_resume, ManagedStreamHandlerPair,
};

#[cfg(feature = "vmess")]
#[derive(Debug, Default)]
pub(crate) struct VmessAdapter {
    runtime: VmessTransportRuntime,
}

#[cfg(feature = "vmess")]
#[derive(Clone, Copy)]
struct VmessOutboundProjection<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    id: &'a str,
    cipher: &'a str,
    mux_concurrency: Option<u32>,
    tls: Option<&'a zero_config::ClientTlsConfig>,
    ws: Option<&'a zero_config::WebSocketConfig>,
    grpc: Option<&'a zero_config::GrpcConfig>,
}

#[cfg(feature = "vmess")]
impl<'a> VmessOutboundProjection<'a> {
    fn from_leaf(leaf: ResolvedLeafOutbound<'a>) -> Option<Self> {
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
        Some(Self {
            tag,
            server,
            port,
            id,
            cipher,
            mux_concurrency,
            tls,
            ws,
            grpc,
        })
    }

    fn endpoint(&self) -> (&'a str, u16) {
        (self.server, self.port)
    }

    fn build_options(
        &self,
    ) -> VmessOutboundBuildOptionsRef<
        'a,
        zero_config::ClientTlsConfig,
        zero_config::WebSocketConfig,
        zero_config::GrpcConfig,
    > {
        VmessOutboundBuildOptionsRef {
            tag: self.tag,
            server: self.server,
            port: self.port,
            protocol: VmessOutboundOptionsRef {
                id: self.id,
                cipher: self.cipher,
                mux_concurrency: self.mux_concurrency,
            },
            tls: self.tls,
            ws: self.ws,
            grpc: self.grpc,
        }
    }
}

#[cfg(feature = "vmess")]
const TCP_PATH: TcpPathCategory = TcpPathCategory::Session;

#[cfg(feature = "vmess")]
impl VmessAdapter {
    pub(crate) fn claim_outbound_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafClaim<'a>> {
        let projection = VmessOutboundProjection::from_leaf(leaf)?;
        let runtime = OutboundLeafRuntime::proxy(
            projection.tag,
            projection.server,
            projection.port,
            TCP_PATH,
        );
        let endpoint = Some(projection.endpoint());
        let tcp_runtime = self.runtime.clone();
        let udp_runtime = self.runtime.clone();
        Some(OutboundLeafClaim {
            runtime: runtime.clone(),
            tcp: claim_transport_tcp_leaf(endpoint, move |source_dir| {
                VmessOutboundLeaf::from_options_refs(
                    source_dir,
                    projection.build_options(),
                    &tcp_runtime,
                )
            }),
            udp: Some(claim_transport_udp_leaf(endpoint, move |source_dir| {
                VmessOutboundLeaf::from_options_refs(
                    source_dir,
                    projection.build_options(),
                    &udp_runtime,
                )
            })),
            packet_path: None,
        })
    }
}

#[cfg(feature = "vmess")]
impl NamedProtocolAdapter for VmessAdapter {
    const PROTOCOL_NAME: &'static str = "vmess";
    const FEATURE_NAME: &'static str = "vmess";
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
        self.runtime.on_config_reloaded();
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
        let request = match &inbound.protocol {
            InboundProtocolConfig::Vmess {
                users,
                tls,
                ws,
                grpc,
            } => VmessInboundListenerRequest::from_options_refs(
                source_dir,
                VmessInboundOptionsRef {
                    users: users.iter().map(|user| VmessInboundUserRef {
                        id: user.id.as_str(),
                        cipher: user.cipher.as_str(),
                        credential_id: user.credential_id.as_deref(),
                        principal_key: user.principal_key.as_deref(),
                        up_bps: user.up_bps,
                        down_bps: user.down_bps,
                    }),
                    tls: tls.as_deref(),
                    ws: ws.as_deref(),
                    grpc: grpc.as_deref(),
                },
            )
            .map_err(EngineError::from)?,
            _ => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "vmess inbound listener received non-vmess inbound config",
                )));
            }
        };
        Ok(listener::prepare(request))
    }
}

#[cfg(feature = "vmess")]
impl TcpOutboundCapability for VmessAdapter {}

#[cfg(feature = "vmess")]
impl UdpFlowCapability for VmessAdapter {}

#[cfg(feature = "vmess")]
impl ManagedUdpHandlerProvider for VmessAdapter {
    fn managed_stream_udp_handlers(&self) -> Option<ManagedStreamHandlerPair> {
        Some(managed_stream_udp_handler_for_resume::<
            <VmessOutboundLeaf as ProtocolManagedStreamUdpLeafOps>::Resume,
        >())
    }
}

#[cfg(feature = "vmess")]
impl UdpPacketPathCapability for VmessAdapter {}
