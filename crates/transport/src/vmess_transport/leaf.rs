use std::future::Future;

use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

use crate::outbound_leaf::{
    clone_socket_opener, ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
};
use crate::transport_plan::{direct_stream_opener, relay_stream_opener};
use crate::StreamTraffic;

use super::managed_udp::VmessManagedUdpFlowResume;
use super::outbound::OwnedVmessOutboundTransportPlan;

#[cfg(feature = "vmess")]
#[derive(Clone)]
pub struct VmessOutboundLeaf<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    transport: OwnedVmessOutboundTransportPlan,
    protocol: vmess::outbound::PreparedVmessOutboundRequestBundle,
}

#[cfg(feature = "vmess")]
impl<'a> VmessOutboundLeaf<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        transport: OwnedVmessOutboundTransportPlan,
        protocol: vmess::outbound::PreparedVmessOutboundRequestBundle,
    ) -> Self {
        Self {
            tag,
            server,
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
        mux_pool: &vmess::mux::VmessMuxConnectionPool,
        open_socket: OpenSocket,
    ) -> Result<vmess::outbound::VmessTcpStreamOpen, EngineError>
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
        protocol
            .open_tcp_relay_hop_with_transport(session, relay_stream_opener(&transport, stream))
            .await
            .map(TcpRelayStream::new)
    }

    pub(super) fn direct_udp_resume(
        &self,
        mux_pool: vmess::mux::VmessMuxConnectionPool,
    ) -> VmessManagedUdpFlowResume {
        VmessManagedUdpFlowResume::new(
            mux_pool,
            self.protocol.udp_direct_flow_plan(),
            self.owned_transport_plan(),
        )
    }

    pub(super) fn relay_final_hop_udp_resume(
        &self,
        mux_pool: vmess::mux::VmessMuxConnectionPool,
    ) -> VmessManagedUdpFlowResume {
        VmessManagedUdpFlowResume::new(
            mux_pool,
            self.protocol.udp_relay_flow_plan(),
            self.owned_transport_plan(),
        )
    }
}

impl ProtocolTransportLeaf for VmessOutboundLeaf<'_> {
    fn tag(&self) -> &str {
        self.tag
    }

    fn server(&self) -> &str {
        self.server
    }

    fn port(&self) -> u16 {
        self.port
    }
}

impl ProtocolTcpTransportOpenResult for vmess::outbound::VmessTcpStreamOpen {
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
