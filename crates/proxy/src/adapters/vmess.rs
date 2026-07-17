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
use zero_engine::EngineError;
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata, ProtocolUdpFlowLeaf};

use crate::adapters::identity::{
    named_protocol_supports_inbound, named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    claim_transport_tcp_leaf, claim_transport_udp_leaf, InboundListenerCapability,
    ManagedUdpHandlerProvider, OutboundLeafClaim, OutboundLeafInput, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
use crate::runtime::transport_leaf::{
    ProxyTransportLeaf, ProxyTransportTcpLeaf, ProxyTransportUdpLeaf,
};
#[cfg(feature = "vmess")]
use crate::runtime::udp_flow::managed::{
    bridge::managed_stream_udp_handler_for_resume, ManagedStreamConnectorParts,
    ManagedStreamHandlerPair, ManagedTupleUdpFlowConnection, ManagedTupleUdpResume,
    ManagedTupleUdpResumeConnector,
};

#[cfg(feature = "vmess")]
#[derive(Debug, Default)]
pub(crate) struct VmessAdapter {
    runtime: VmessTransportRuntime,
}

#[cfg(feature = "vmess")]
impl ProxyTransportLeaf for VmessOutboundLeaf {
    fn tag(&self) -> &str {
        self.tag()
    }

    fn server(&self) -> &str {
        self.server()
    }

    fn port(&self) -> u16 {
        self.port()
    }
}

#[cfg(feature = "vmess")]
#[async_trait::async_trait]
impl ProxyTransportTcpLeaf for VmessOutboundLeaf {
    const TCP_CONNECT_STAGE: &'static str = "connect_upstream_vmess";
    const TCP_INVALID_CONNECT_CONFIG: &'static str = "invalid vmess tcp config";
    const TCP_INVALID_RELAY_CONFIG: &'static str = "invalid vmess tcp relay config";

    async fn open_tcp_stream(
        &self,
        services: crate::protocol_registry::TcpRuntimeServices,
        session: &zero_core::Session,
    ) -> Result<
        (
            crate::transport::TcpRelayStream,
            zero_transport::StreamTraffic,
        ),
        zero_transport::RuntimeError,
    > {
        let opened = VmessOutboundLeaf::open_tcp_stream(self, session, move |server, port| {
            let services = services.clone();
            let server = server.to_owned();
            async move {
                services
                    .connect_upstream_owned(server, port)
                    .await
                    .map_err(zero_transport::RuntimeError::from)
            }
        })
        .await?;
        let (stream, handshake_bytes) = opened.into_parts();
        Ok((
            crate::transport::TcpRelayStream::new(stream),
            zero_transport::StreamTraffic {
                read_bytes: 0,
                written_bytes: handshake_bytes,
            },
        ))
    }

    async fn open_tcp_relay_hop(
        &self,
        stream: crate::transport::TcpRelayStream,
        session: &zero_core::Session,
    ) -> Result<crate::transport::TcpRelayStream, zero_transport::RuntimeError> {
        VmessOutboundLeaf::open_tcp_relay_hop(self, stream, session).await
    }
}

#[cfg(feature = "vmess")]
impl ProxyTransportUdpLeaf for VmessOutboundLeaf {
    type RuntimeResume = ManagedTupleUdpResume<::vmess::transport::VmessManagedUdpFlowResume>;

    const UDP_DIRECT_STAGE: &'static str = "udp_vmess_leaf";
    const UDP_INVALID_CONFIG: &'static str = "invalid vmess udp config";
    const UDP_RELAY_FINAL_STAGE: &'static str = "udp_vmess_relay_final_leaf";

    fn direct_udp_resume(&self) -> Self::RuntimeResume {
        ManagedTupleUdpResume::new(ProtocolUdpFlowLeaf::direct_udp_resume(self))
    }

    fn relay_final_hop_udp_resume(&self) -> Self::RuntimeResume {
        ManagedTupleUdpResume::new(ProtocolUdpFlowLeaf::relay_final_hop_udp_resume(self))
    }
}

#[cfg(feature = "vmess")]
fn outbound_options<'a>(
    tag: &'a str,
    endpoint: (&'a str, u16),
    protocol: &'a OutboundProtocolConfig,
) -> Option<
    VmessOutboundBuildOptionsRef<
        'a,
        zero_config::ClientTlsConfig,
        zero_config::WebSocketConfig,
        zero_config::GrpcConfig,
    >,
> {
    let OutboundProtocolConfig::Vmess {
        id,
        cipher,
        mux_concurrency,
        tls,
        ws,
        grpc,
        ..
    } = protocol
    else {
        return None;
    };
    Some(VmessOutboundBuildOptionsRef {
        tag,
        server: endpoint.0,
        port: endpoint.1,
        protocol: VmessOutboundOptionsRef {
            id,
            cipher,
            mux_concurrency: *mux_concurrency,
        },
        tls: tls.as_deref(),
        ws: ws.as_deref(),
        grpc: grpc.as_deref(),
    })
}

#[cfg(feature = "vmess")]
const TCP_PATH: TcpPathCategory = TcpPathCategory::Session;

#[cfg(feature = "vmess")]
#[async_trait::async_trait]
impl ManagedTupleUdpResumeConnector for ::vmess::transport::VmessManagedUdpFlowResume {
    type ConnectorFlow = ::vmess::transport::VmessManagedUdpConnectorFlow;
    type Connection = ::vmess::udp::VmessUdpFlowConnection;

    const ESTABLISH_STAGE: &'static str = "vmess_establish";
    const RELAY_UPSTREAM_STAGE: &'static str = "vmess_relay_upstream";
    const RELAY_ESTABLISH_STAGE: &'static str = "vmess_relay_establish";
    const RELAY_SEND_STAGE: &'static str = "vmess_relay_send";
    const MISMATCH_STAGE: &'static str = "udp_vmess_resume";
    const MISMATCH_MESSAGE: &'static str = "expected VMess UDP flow resume";

    fn connector_flow(&self, server: &str, port: u16, session_id: u64) -> Self::ConnectorFlow {
        ::vmess::transport::VmessManagedUdpFlowResume::connector_flow(
            self, server, port, session_id,
        )
    }

    async fn open_direct(
        &self,
        services: crate::protocol_registry::UdpRuntimeServices,
        session: &zero_core::Session,
    ) -> Result<Self::Connection, EngineError> {
        self.open_direct_connection(session, move |server, port| {
            let services = services.clone();
            let server = server.to_owned();
            async move { services.connect_upstream(&server, port).await }
        })
        .await
        .map_err(EngineError::from)
    }

    async fn open_relay(
        &self,
        stream: crate::transport::TcpRelayStream,
        session: &zero_core::Session,
        _tls_server_name: Option<&str>,
    ) -> Result<Self::Connection, EngineError> {
        self.open_relay_connection(stream, session)
            .await
            .map_err(EngineError::from)
    }
}

#[cfg(feature = "vmess")]
impl ManagedStreamConnectorParts for ::vmess::transport::VmessManagedUdpConnectorFlow {
    fn into_managed_connector_parts(self) -> (String, bool) {
        self.into_parts()
    }
}

#[cfg(feature = "vmess")]
#[async_trait::async_trait]
impl ManagedTupleUdpFlowConnection for ::vmess::udp::VmessUdpFlowConnection {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        ::vmess::udp::VmessUdpFlowConnection::send(self, target, port, payload)
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))
    }

    fn subscribe_responses(
        &self,
    ) -> tokio::sync::broadcast::Receiver<(zero_core::Address, u16, Vec<u8>)> {
        ::vmess::udp::VmessUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message(&self) -> &'static str {
        "vmess upstream closed"
    }
}

#[cfg(feature = "vmess")]
impl VmessAdapter {
    pub(crate) fn claim_outbound_leaf_impl<'a>(
        &self,
        input: OutboundLeafInput<'a>,
    ) -> Option<OutboundLeafClaim<'a>> {
        let OutboundLeafInput::Proxy { outbound, endpoint } = input else {
            return None;
        };
        let options = outbound_options(outbound.tag(), endpoint, &outbound.protocol)?;
        let endpoint = Some(endpoint);
        let tcp_runtime = self.runtime.clone();
        let udp_runtime = self.runtime.clone();
        Some(OutboundLeafClaim {
            tcp_path: TCP_PATH,
            tcp: claim_transport_tcp_leaf(endpoint, move |source_dir| {
                VmessOutboundLeaf::from_options_refs(source_dir, options, &tcp_runtime)
            }),
            udp: Some(claim_transport_udp_leaf(endpoint, move |source_dir| {
                VmessOutboundLeaf::from_options_refs(source_dir, options, &udp_runtime)
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
            <VmessOutboundLeaf as ProxyTransportUdpLeaf>::RuntimeResume,
        >())
    }
}

#[cfg(feature = "vmess")]
impl UdpPacketPathCapability for VmessAdapter {}
