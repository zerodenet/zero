use std::future::Future;
use std::path::Path;

use zero_core::Session;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};
use zero_traits::{ClientTlsProfile, GrpcTransportProfile, WebSocketTransportProfile};
use zero_transport::outbound_leaf::{
    clone_socket_opener, ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
};
use zero_transport::transport_plan::{direct_stream_opener, relay_stream_opener};
use zero_transport::RuntimeError;
use zero_transport::StreamTraffic;

use super::managed_udp::VmessManagedUdpFlowResume;
use super::outbound::OwnedVmessOutboundTransportPlan;

#[derive(Clone)]
pub struct OwnedVmessOutboundLeafConfig {
    tag: String,
    server: String,
    port: u16,
    transport: OwnedVmessOutboundTransportPlan,
    protocol: crate::outbound::PreparedVmessOutboundRequestBundle,
}

impl OwnedVmessOutboundLeafConfig {
    #[allow(clippy::too_many_arguments)]
    pub fn from_config_refs<TTls, TWs, TGrpc>(
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
}

impl VmessOutboundLeaf {
    pub fn new(
        tag: &str,
        server: &str,
        port: u16,
        transport: OwnedVmessOutboundTransportPlan,
        protocol: crate::outbound::PreparedVmessOutboundRequestBundle,
    ) -> Self {
        Self {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            protocol,
            transport,
        }
    }

    fn owned_transport_plan(&self) -> OwnedVmessOutboundTransportPlan {
        self.transport.clone()
    }

    pub(super) async fn open_tcp_stream<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        mux_pool: &crate::mux::VmessMuxConnectionPool,
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
        protocol
            .open_tcp_relay_hop_with_transport(session, relay_stream_opener(&transport, stream))
            .await
            .map(TcpRelayStream::new)
    }

    pub(super) fn direct_udp_resume(
        &self,
        mux_pool: crate::mux::VmessMuxConnectionPool,
    ) -> VmessManagedUdpFlowResume {
        VmessManagedUdpFlowResume::new(
            mux_pool,
            self.protocol.udp_direct_flow_plan(),
            self.owned_transport_plan(),
        )
    }

    pub(super) fn relay_final_hop_udp_resume(
        &self,
        mux_pool: crate::mux::VmessMuxConnectionPool,
    ) -> VmessManagedUdpFlowResume {
        VmessManagedUdpFlowResume::new(
            mux_pool,
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
        } = config;
        Self {
            tag,
            server,
            port,
            transport,
            protocol,
        }
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
