#[cfg(feature = "vless")]
use ::vless::transport::{
    VlessOutboundLeaf, VlessOutboundOptionsRef, VlessQuicBindOptionsRef, VlessQuicClientOptionsRef,
    VlessRealityClientOptionsRef, VlessTransportRuntime,
};
#[cfg(feature = "vless")]
use async_trait::async_trait;
#[cfg(feature = "vless")]
mod listener;
#[cfg(feature = "vless")]
use zero_config::InboundConfig;
use zero_config::{InboundProtocolConfig, OutboundProtocolConfig};
#[cfg(feature = "vless")]
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};
#[cfg(feature = "vless")]
use zero_transport::managed_udp::ProtocolManagedStreamUdpLeafOps;

use crate::adapters::identity::{
    named_protocol_supports_inbound, named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    bind_transport_inbound, claim_relay_two_stream_transport_udp_leaf, claim_transport_tcp_leaf,
    proxy_leaf_runtime, BoundInbound, ClaimedTcpOutboundLeaf, ClaimedUdpFlowLeaf,
    InboundListenerCapability, ManagedUdpHandlerProvider, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
#[cfg(feature = "vless")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_resume, ManagedStreamHandlerPair,
};

#[cfg(feature = "vless")]
#[derive(Debug, Default)]
pub(crate) struct VlessAdapter {
    runtime: VlessTransportRuntime,
}

#[cfg(feature = "vless")]
const TCP_PATH: TcpPathCategory = TcpPathCategory::Tunnel;

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
        self.runtime.on_config_reloaded();
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
        let plan = self.runtime.build_inbound_bind_plan(
            source_dir,
            quic.as_deref().map(|quic| VlessQuicBindOptionsRef {
                cert_path: quic.cert_path.as_deref(),
                key_path: quic.key_path.as_deref(),
            }),
        );
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
        listener::prepare(self.runtime.clone(), inbound, source_dir)
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
        Some(claim_transport_tcp_leaf(Some((server, port)), runtime, {
            let transport_runtime = self.runtime.clone();
            move |source_dir| {
                transport_runtime.build_outbound_leaf(
                    source_dir,
                    tag,
                    server,
                    port,
                    VlessOutboundOptionsRef {
                        id,
                        flow,
                        mux_concurrency,
                        reality: reality.map(|reality| VlessRealityClientOptionsRef {
                            public_key: reality.public_key.as_str(),
                            short_id: reality.short_id.as_str(),
                            server_name: reality.server_name.as_deref(),
                            cipher_suites: reality.cipher_suites.as_slice(),
                        }),
                        quic: quic.map(|quic| VlessQuicClientOptionsRef {
                            server_name: quic.server_name.as_deref(),
                            insecure: quic.insecure,
                            ca_cert_path: quic.ca_cert_path.as_deref(),
                        }),
                    },
                    tls,
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                )
            }
        }))
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
        Some(claim_relay_two_stream_transport_udp_leaf(
            Some((server, port)),
            {
                let transport_runtime = self.runtime.clone();
                move |source_dir| {
                    transport_runtime.build_outbound_leaf(
                        source_dir,
                        tag,
                        server,
                        port,
                        VlessOutboundOptionsRef {
                            id,
                            flow,
                            mux_concurrency,
                            reality: reality.map(|reality| VlessRealityClientOptionsRef {
                                public_key: reality.public_key.as_str(),
                                short_id: reality.short_id.as_str(),
                                server_name: reality.server_name.as_deref(),
                                cipher_suites: reality.cipher_suites.as_slice(),
                            }),
                            quic: quic.map(|quic| VlessQuicClientOptionsRef {
                                server_name: quic.server_name.as_deref(),
                                insecure: quic.insecure,
                                ca_cert_path: quic.ca_cert_path.as_deref(),
                            }),
                        },
                        tls,
                        ws,
                        grpc,
                        h2,
                        http_upgrade,
                        split_http,
                    )
                }
            },
        ))
    }
}

#[cfg(feature = "vless")]
impl ManagedUdpHandlerProvider for VlessAdapter {
    fn managed_stream_udp_handlers(&self) -> Option<ManagedStreamHandlerPair> {
        Some(managed_stream_udp_handler_for_resume::<
            <VlessOutboundLeaf as ProtocolManagedStreamUdpLeafOps>::Resume,
        >())
    }
}

#[cfg(feature = "vless")]
impl UdpPacketPathCapability for VlessAdapter {}
