use std::future::Future;

use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

use crate::outbound_leaf::{
    clone_socket_opener, ProtocolRelayTwoStreamTransportLeaf, ProtocolTcpTransportOpenResult,
    ProtocolTransportLeaf,
};
use crate::transport_plan::{direct_stream_opener, relay_stream_opener};
use crate::StreamTraffic;

use super::managed_udp::VlessManagedUdpFlowResume;
use super::outbound::OwnedVlessOutboundTransportPlan;

#[derive(Clone)]
pub struct VlessOutboundLeaf<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    transport: OwnedVlessOutboundTransportPlan,
    protocol: vless::outbound::PreparedVlessOutboundRequestBundle,
}

impl<'a> VlessOutboundLeaf<'a> {
    pub(super) fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        transport: OwnedVlessOutboundTransportPlan,
        protocol: vless::outbound::PreparedVlessOutboundRequestBundle,
    ) -> Self {
        Self {
            tag,
            server,
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
    ) -> Result<TcpRelayStream, EngineError> {
        self.transport
            .build_relay_two_stream_udp_transport(post_stream, get_stream)
            .await
    }

    pub(super) async fn open_tcp_stream<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        mux_pool: &vless::mux_pool::MuxConnectionPool,
        open_socket: OpenSocket,
    ) -> Result<vless::outbound::VlessTcpStreamOpen, EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send,
    {
        let protocol = self.protocol.clone();
        let transport = self.owned_transport_plan();
        let open_socket = clone_socket_opener(open_socket);
        let direct_transport = direct_stream_opener(&transport, open_socket.clone());
        protocol
            .open_tcp_stream_with_transport_or_mux(
                session,
                self.server,
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
    ) -> Result<TcpRelayStream, EngineError> {
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
        mux_pool: vless::mux_pool::MuxConnectionPool,
    ) -> VlessManagedUdpFlowResume {
        VlessManagedUdpFlowResume::new(
            mux_pool,
            self.protocol.udp_direct_flow_plan(),
            self.owned_transport_plan(),
        )
    }

    pub(super) fn relay_two_stream_udp_resume(
        &self,
        mux_pool: vless::mux_pool::MuxConnectionPool,
    ) -> VlessManagedUdpFlowResume {
        VlessManagedUdpFlowResume::new(
            mux_pool,
            self.protocol.udp_relay_paired_transport_plan(),
            self.owned_transport_plan(),
        )
    }

    pub(super) fn relay_final_hop_udp_resume(
        &self,
        mux_pool: vless::mux_pool::MuxConnectionPool,
    ) -> VlessManagedUdpFlowResume {
        VlessManagedUdpFlowResume::new(
            mux_pool,
            self.protocol.udp_relay_final_hop_plan(),
            self.owned_transport_plan(),
        )
    }
}

impl ProtocolTransportLeaf for VlessOutboundLeaf<'_> {
    fn tag(&self) -> &str {
        self.tag
    }

    fn server(&self) -> &str {
        self.server
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn validate_udp_relay_final_hop(&self) -> Result<(), EngineError> {
        if self.owned_transport_plan().uses_quic() {
            return Err(zero_core::Error::Unsupported(
                "VLESS QUIC final hop over TCP relay chain is not supported",
            )
            .into());
        }
        Ok(())
    }
}

impl ProtocolTcpTransportOpenResult for vless::outbound::VlessTcpStreamOpen {
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
impl<'a> ProtocolRelayTwoStreamTransportLeaf for VlessOutboundLeaf<'a> {
    async fn open_relay_two_stream_udp_transport(
        &self,
        post_stream: TcpRelayStream,
        get_stream: TcpRelayStream,
    ) -> Result<TcpRelayStream, EngineError> {
        VlessOutboundLeaf::build_relay_two_stream_udp_transport(self, post_stream, get_stream).await
    }

    fn needs_relay_two_streams(&self) -> bool {
        VlessOutboundLeaf::relay_needs_two_streams(self)
    }
}
