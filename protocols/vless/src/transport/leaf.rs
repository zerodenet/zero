use std::future::Future;
use std::path::Path;

use zero_core::Session;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};
use zero_traits::{
    ClientTlsProfile, GrpcTransportProfile, H2TransportProfile, HttpUpgradeTransportProfile,
    SplitHttpTransportProfile, WebSocketTransportProfile,
};
use zero_transport::RuntimeError;

use zero_transport::outbound_leaf::{
    clone_socket_opener, ProtocolRelayTwoStreamTransportLeaf, ProtocolTcpTransportOpenResult,
    ProtocolTransportLeaf,
};
use zero_transport::transport_plan::{direct_stream_opener, relay_stream_opener};
use zero_transport::StreamTraffic;

use super::managed_udp::VlessManagedUdpFlowResume;
use super::outbound::OwnedVlessOutboundTransportPlan;
use super::profile::{VlessQuicClientProfile, VlessRealityClientProfile};

#[derive(Clone)]
struct OwnedVlessOutboundLeafConfig {
    tag: String,
    server: String,
    port: u16,
    transport: OwnedVlessOutboundTransportPlan,
    protocol: crate::outbound::PreparedVlessOutboundRequestBundle,
}

impl OwnedVlessOutboundLeafConfig {
    #[allow(clippy::too_many_arguments)]
    fn from_config_refs<TTls, TWs, TGrpc, TH2, THttp, TSplit>(
        source_dir: Option<&Path>,
        tag: &str,
        server: &str,
        port: u16,
        id: &str,
        flow: Option<&str>,
        mux_concurrency: Option<u32>,
        tls: Option<&TTls>,
        reality: Option<&VlessRealityClientProfile>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
        h2: Option<&TH2>,
        http_upgrade: Option<&THttp>,
        split_http: Option<&TSplit>,
        quic: Option<&VlessQuicClientProfile>,
    ) -> Result<Self, zero_core::Error>
    where
        TTls: ClientTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
        TH2: H2TransportProfile + ?Sized,
        THttp: HttpUpgradeTransportProfile + ?Sized,
        TSplit: SplitHttpTransportProfile + ?Sized,
    {
        let transport = OwnedVlessOutboundTransportPlan::from_profile_refs(
            source_dir,
            server,
            port,
            tls,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            quic,
        );
        let protocol =
            crate::outbound::PreparedVlessOutboundRequestBundle::from_config_with_transport_hints(
                id,
                flow,
                mux_concurrency,
                transport.mux_transport_hints(),
            )?;
        Ok(Self {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            transport,
            protocol,
        })
    }
}

#[derive(Clone)]
pub struct VlessOutboundLeaf {
    tag: String,
    server: String,
    port: u16,
    transport: OwnedVlessOutboundTransportPlan,
    protocol: crate::outbound::PreparedVlessOutboundRequestBundle,
}

impl VlessOutboundLeaf {
    #[allow(clippy::too_many_arguments)]
    pub fn from_config_refs<TTls, TWs, TGrpc, TH2, THttp, TSplit>(
        source_dir: Option<&Path>,
        tag: &str,
        server: &str,
        port: u16,
        id: &str,
        flow: Option<&str>,
        mux_concurrency: Option<u32>,
        tls: Option<&TTls>,
        reality: Option<&VlessRealityClientProfile>,
        ws: Option<&TWs>,
        grpc: Option<&TGrpc>,
        h2: Option<&TH2>,
        http_upgrade: Option<&THttp>,
        split_http: Option<&TSplit>,
        quic: Option<&VlessQuicClientProfile>,
    ) -> Result<Self, zero_core::Error>
    where
        TTls: ClientTlsProfile + ?Sized,
        TWs: WebSocketTransportProfile + ?Sized,
        TGrpc: GrpcTransportProfile + ?Sized,
        TH2: H2TransportProfile + ?Sized,
        THttp: HttpUpgradeTransportProfile + ?Sized,
        TSplit: SplitHttpTransportProfile + ?Sized,
    {
        OwnedVlessOutboundLeafConfig::from_config_refs(
            source_dir,
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
        )
        .map(Into::into)
    }

    pub(super) fn new(
        tag: &str,
        server: &str,
        port: u16,
        transport: OwnedVlessOutboundTransportPlan,
        protocol: crate::outbound::PreparedVlessOutboundRequestBundle,
    ) -> Self {
        Self {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            protocol,
            transport,
        }
    }

    pub(super) fn relay_needs_two_streams(&self) -> bool {
        self.transport.relay_needs_two_streams()
    }

    fn uses_deferred_tcp_response(&self) -> bool {
        self.transport.uses_deferred_tcp_response()
    }

    fn owned_transport_plan(&self) -> OwnedVlessOutboundTransportPlan {
        self.transport.clone()
    }

    async fn build_relay_two_stream_udp_transport(
        &self,
        post_stream: TcpRelayStream,
        get_stream: TcpRelayStream,
    ) -> Result<TcpRelayStream, RuntimeError> {
        self.transport
            .build_relay_two_stream_udp_transport(post_stream, get_stream)
            .await
    }

    pub(super) async fn open_tcp_stream<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        mux_pool: &crate::mux_pool::MuxConnectionPool,
        open_socket: OpenSocket,
    ) -> Result<crate::outbound::VlessTcpStreamOpen, RuntimeError>
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
                self.uses_deferred_tcp_response(),
                mux_pool,
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
        let quic_requested = transport.uses_quic();
        protocol
            .open_tcp_relay_hop_with_transport(
                session,
                quic_requested,
                relay_stream_opener(&transport, stream),
            )
            .await
            .map(TcpRelayStream::new)
    }

    pub(super) fn direct_udp_resume(
        &self,
        mux_pool: crate::mux_pool::MuxConnectionPool,
    ) -> VlessManagedUdpFlowResume {
        VlessManagedUdpFlowResume::new(
            mux_pool,
            self.protocol.udp_direct_flow_plan(),
            self.owned_transport_plan(),
        )
    }

    pub(super) fn relay_two_stream_udp_resume(
        &self,
        mux_pool: crate::mux_pool::MuxConnectionPool,
    ) -> VlessManagedUdpFlowResume {
        VlessManagedUdpFlowResume::new(
            mux_pool,
            self.protocol.udp_relay_paired_transport_plan(),
            self.owned_transport_plan(),
        )
    }

    pub(super) fn relay_final_hop_udp_resume(
        &self,
        mux_pool: crate::mux_pool::MuxConnectionPool,
    ) -> VlessManagedUdpFlowResume {
        VlessManagedUdpFlowResume::new(
            mux_pool,
            self.protocol.udp_relay_final_hop_plan(),
            self.owned_transport_plan(),
        )
    }
}

impl From<OwnedVlessOutboundLeafConfig> for VlessOutboundLeaf {
    fn from(config: OwnedVlessOutboundLeafConfig) -> Self {
        let OwnedVlessOutboundLeafConfig {
            tag,
            server,
            port,
            transport,
            protocol,
        } = config;
        Self::new(&tag, &server, port, transport, protocol)
    }
}

impl ProtocolTransportLeaf for VlessOutboundLeaf {
    fn tag(&self) -> &str {
        &self.tag
    }

    fn server(&self) -> &str {
        &self.server
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn validate_udp_relay_final_hop(&self) -> Result<(), RuntimeError> {
        if self.owned_transport_plan().uses_quic() {
            return Err(zero_core::Error::Unsupported(
                "VLESS QUIC final hop over TCP relay chain is not supported",
            )
            .into());
        }
        Ok(())
    }
}

impl ProtocolTcpTransportOpenResult for crate::outbound::VlessTcpStreamOpen {
    fn into_proxied_stream_parts(self) -> (TcpRelayStream, StreamTraffic) {
        let (stream, handshake_written_bytes, handshake_read_bytes) = self.into_parts();
        (
            TcpRelayStream::new(stream),
            StreamTraffic {
                read_bytes: handshake_read_bytes,
                written_bytes: handshake_written_bytes,
            },
        )
    }
}

#[async_trait::async_trait]
impl ProtocolRelayTwoStreamTransportLeaf for VlessOutboundLeaf {
    async fn open_relay_two_stream_udp_transport(
        &self,
        post_stream: TcpRelayStream,
        get_stream: TcpRelayStream,
    ) -> Result<TcpRelayStream, RuntimeError> {
        VlessOutboundLeaf::build_relay_two_stream_udp_transport(self, post_stream, get_stream).await
    }

    fn needs_relay_two_streams(&self) -> bool {
        VlessOutboundLeaf::relay_needs_two_streams(self)
    }
}
