use std::future::Future;
use std::path::Path;

use zero_core::Session;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};
use zero_traits::{ClientTlsProfile, GrpcTransportProfile, WebSocketTransportProfile};
use zero_transport::managed_udp::{ManagedTupleUdpResume, ProtocolManagedStreamUdpLeafOps};
use zero_transport::outbound_leaf::{
    clone_socket_opener, ProtocolTcpTransportLeafMetadata, ProtocolTcpTransportLeafOps,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf, ProtocolUdpTransportLeafMetadata,
};
use zero_transport::transport_plan::{direct_stream_opener, relay_stream_opener};
use zero_transport::RuntimeError;
use zero_transport::StreamTraffic;

use super::managed_udp::{VmessManagedStreamUdpResume, VmessManagedUdpFlowResume};
use super::outbound::OwnedVmessOutboundTransportPlan;

#[derive(Clone)]
struct OwnedVmessOutboundLeafConfig {
    tag: String,
    server: String,
    port: u16,
    transport: OwnedVmessOutboundTransportPlan,
    protocol: crate::outbound::PreparedVmessOutboundRequestBundle,
    mux_pool: crate::mux::VmessMuxConnectionPool,
}

impl OwnedVmessOutboundLeafConfig {
    #[allow(clippy::too_many_arguments)]
    fn from_profile_refs<TTls, TWs, TGrpc>(
        source_dir: Option<&Path>,
        tag: &str,
        server: &str,
        port: u16,
        id: &str,
        cipher: &str,
        mux_concurrency: Option<u32>,
        tls: Option<&TTls>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
        mux_pool: crate::mux::VmessMuxConnectionPool,
    ) -> Result<Self, zero_core::Error>
    where
        TTls: ClientTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
    {
        let transport = OwnedVmessOutboundTransportPlan::from_profile_refs(
            source_dir, server, port, tls, ws, grpc,
        );
        let protocol =
            crate::outbound::PreparedVmessOutboundRequestBundle::from_config_with_transport_hints(
                id,
                cipher,
                mux_concurrency,
                transport.mux_transport_hints(),
            )?;
        Ok(Self {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            transport,
            protocol,
            mux_pool,
        })
    }
}

#[derive(Clone)]
pub struct VmessOutboundLeaf {
    tag: String,
    server: String,
    port: u16,
    transport: OwnedVmessOutboundTransportPlan,
    protocol: crate::outbound::PreparedVmessOutboundRequestBundle,
    mux_pool: crate::mux::VmessMuxConnectionPool,
}

impl VmessOutboundLeaf {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::transport) fn from_profile_refs<TTls, TWs, TGrpc>(
        source_dir: Option<&Path>,
        tag: &str,
        server: &str,
        port: u16,
        id: &str,
        cipher: &str,
        mux_concurrency: Option<u32>,
        tls: Option<&TTls>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
        mux_pool: crate::mux::VmessMuxConnectionPool,
    ) -> Result<Self, zero_core::Error>
    where
        TTls: ClientTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
    {
        OwnedVmessOutboundLeafConfig::from_profile_refs(
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
            mux_pool,
        )
        .map(Into::into)
    }

    pub(super) fn new(
        tag: &str,
        server: &str,
        port: u16,
        transport: OwnedVmessOutboundTransportPlan,
        protocol: crate::outbound::PreparedVmessOutboundRequestBundle,
        mux_pool: crate::mux::VmessMuxConnectionPool,
    ) -> Self {
        Self {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            protocol,
            transport,
            mux_pool,
        }
    }

    fn owned_transport_plan(&self) -> OwnedVmessOutboundTransportPlan {
        self.transport.clone()
    }

    pub(super) async fn open_tcp_stream<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<crate::outbound::VmessTcpStreamOpen, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        let protocol = self.protocol.clone();
        let transport = self.owned_transport_plan();
        let open_socket = clone_socket_opener(open_socket);
        let direct_transport = direct_stream_opener(&transport, open_socket.clone());
        protocol
            .open_tcp_stream_with_transport_or_mux(
                session,
                &self.server,
                self.port,
                &self.mux_pool,
                direct_transport,
            )
            .await
    }

    pub(super) async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError> {
        let protocol = self.protocol.clone();
        let transport = self.owned_transport_plan();
        protocol
            .open_tcp_relay_hop_with_transport(session, relay_stream_opener(&transport, stream))
            .await
            .map(TcpRelayStream::new)
    }

    pub(super) fn direct_udp_resume(&self) -> VmessManagedUdpFlowResume {
        VmessManagedUdpFlowResume::new(
            self.mux_pool.clone(),
            self.protocol.udp_direct_flow_plan(),
            self.owned_transport_plan(),
        )
    }

    pub(super) fn relay_final_hop_udp_resume(&self) -> VmessManagedUdpFlowResume {
        VmessManagedUdpFlowResume::new(
            self.mux_pool.clone(),
            self.protocol.udp_relay_flow_plan(),
            self.owned_transport_plan(),
        )
    }
}

impl From<OwnedVmessOutboundLeafConfig> for VmessOutboundLeaf {
    fn from(config: OwnedVmessOutboundLeafConfig) -> Self {
        let OwnedVmessOutboundLeafConfig {
            tag,
            server,
            port,
            transport,
            protocol,
            mux_pool,
        } = config;
        Self::new(&tag, &server, port, transport, protocol, mux_pool)
    }
}

impl ProtocolTransportLeaf for VmessOutboundLeaf {
    fn tag(&self) -> &str {
        &self.tag
    }

    fn server(&self) -> &str {
        &self.server
    }

    fn port(&self) -> u16 {
        self.port
    }
}

impl ProtocolTcpTransportLeafMetadata for VmessOutboundLeaf {
    const TCP_CONNECT_STAGE: &'static str = "connect_upstream_vmess";
    const TCP_INVALID_CONNECT_CONFIG: &'static str = "invalid vmess tcp config";
    const TCP_INVALID_RELAY_CONFIG: &'static str = "invalid vmess tcp relay config";
}

impl ProtocolUdpTransportLeafMetadata for VmessOutboundLeaf {
    const UDP_DIRECT_STAGE: &'static str = "udp_vmess_leaf";
    const UDP_INVALID_CONFIG: &'static str = "invalid vmess udp config";
    const UDP_RELAY_FINAL_STAGE: &'static str = "udp_vmess_relay_final_leaf";
}

#[async_trait::async_trait]
impl ProtocolTcpTransportLeafOps for VmessOutboundLeaf {
    type Opened = crate::outbound::VmessTcpStreamOpen;

    async fn open_tcp_stream<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<Self::Opened, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        VmessOutboundLeaf::open_tcp_stream(self, session, open_socket).await
    }

    async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError> {
        VmessOutboundLeaf::open_tcp_relay_hop(self, stream, session).await
    }
}

impl ProtocolManagedStreamUdpLeafOps for VmessOutboundLeaf {
    type Resume = VmessManagedStreamUdpResume;

    fn direct_udp_resume(&self) -> Self::Resume {
        ManagedTupleUdpResume::new(VmessOutboundLeaf::direct_udp_resume(self))
    }

    fn relay_final_hop_udp_resume(&self) -> Self::Resume {
        ManagedTupleUdpResume::new(VmessOutboundLeaf::relay_final_hop_udp_resume(self))
    }
}

impl ProtocolTcpTransportOpenResult for crate::outbound::VmessTcpStreamOpen {
    fn into_proxied_stream_parts(self) -> (TcpRelayStream, StreamTraffic) {
        let (stream, handshake_bytes) = self.into_parts();
        (
            TcpRelayStream::new(stream),
            StreamTraffic {
                read_bytes: 0,
                written_bytes: handshake_bytes,
            },
        )
    }
}
