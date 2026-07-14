use std::future::Future;

use zero_core::Session;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};
use zero_transport::managed_udp::{
    ManagedConnectorFlow, ManagedConnectorFlowOps, ManagedTupleUdpConnectionOps,
    ManagedTupleUdpResume, ProtocolManagedTupleUdpFlowResumeConnectionOps,
    ProtocolManagedTupleUdpResumeMetadata,
};
use zero_transport::outbound_leaf::clone_socket_opener;
use zero_transport::transport_plan::{direct_stream_opener, relay_stream_mapper};
use zero_transport::RuntimeError;

use super::outbound::OwnedVmessOutboundTransportPlan;

#[derive(Debug, Clone)]
pub struct VmessManagedUdpFlowResume {
    mux_pool: crate::mux::VmessMuxConnectionPool,
    protocol: crate::udp::PreparedVmessUdpFlowPlan,
    transport: OwnedVmessOutboundTransportPlan,
}

type VmessManagedUdpConnectorFlow = ManagedConnectorFlow<crate::udp::VmessUdpConnectorFlow>;

pub type VmessManagedStreamUdpResume = ManagedTupleUdpResume<VmessManagedUdpFlowResume>;

impl ProtocolManagedTupleUdpResumeMetadata for VmessManagedUdpFlowResume {
    const ESTABLISH_STAGE: &'static str = "vmess_establish";
    const RELAY_UPSTREAM_STAGE: &'static str = "vmess_relay_upstream";
    const RELAY_ESTABLISH_STAGE: &'static str = "vmess_relay_establish";
    const RELAY_SEND_STAGE: &'static str = "vmess_relay_send";
    const MISMATCH_STAGE: &'static str = "udp_vmess_resume";
    const MISMATCH_MESSAGE: &'static str = "expected VMess UDP flow resume";
}

impl VmessManagedUdpFlowResume {
    pub(super) fn new(
        mux_pool: crate::mux::VmessMuxConnectionPool,
        protocol: crate::udp::PreparedVmessUdpFlowPlan,
        transport: OwnedVmessOutboundTransportPlan,
    ) -> Self {
        Self {
            mux_pool,
            protocol,
            transport,
        }
    }

    fn connector_flow(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> VmessManagedUdpConnectorFlow {
        ManagedConnectorFlow(self.protocol.connector_flow(server, port, session_id))
    }

    async fn open_direct_connection<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<crate::udp::VmessUdpFlowConnection, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        let transport = self.transport.clone();
        let open_socket = clone_socket_opener(open_socket);
        let direct_transport = direct_stream_opener(&transport, open_socket.clone());
        self.protocol
            .open_udp_flow_with_transport_or_mux(
                session,
                self.transport.server(),
                self.transport.port(),
                &self.mux_pool,
                direct_transport,
            )
            .await
    }

    async fn open_relay_connection(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<crate::udp::VmessUdpFlowConnection, RuntimeError> {
        let transport = self.transport.clone();
        self.protocol
            .open_relay_udp_flow_with_transport(session, stream, relay_stream_mapper(&transport))
            .await
    }
}

impl ManagedConnectorFlowOps for crate::udp::VmessUdpConnectorFlow {
    fn into_managed_connector_parts(self) -> (String, bool) {
        crate::udp::VmessUdpConnectorFlow::into_parts(self)
    }
}

#[async_trait::async_trait]
impl ProtocolManagedTupleUdpFlowResumeConnectionOps for VmessManagedUdpFlowResume {
    type ConnectorFlow = VmessManagedUdpConnectorFlow;
    type RawConnection = crate::udp::VmessUdpFlowConnection;

    fn connector_flow_for_resume(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Self::ConnectorFlow {
        VmessManagedUdpFlowResume::connector_flow(self, server, port, session_id)
    }

    async fn open_direct_protocol_connection<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<Self::RawConnection, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        VmessManagedUdpFlowResume::open_direct_connection(self, session, open_socket).await
    }

    async fn open_relay_protocol_connection(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        _tls_server_name: Option<&str>,
    ) -> Result<Self::RawConnection, RuntimeError> {
        VmessManagedUdpFlowResume::open_relay_connection(self, stream, session).await
    }
}

#[async_trait::async_trait]
impl ManagedTupleUdpConnectionOps for crate::udp::VmessUdpFlowConnection {
    type SendError = zero_core::Error;

    async fn send_protocol_packet(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Self::SendError> {
        crate::udp::VmessUdpFlowConnection::send(self, target, port, payload).await
    }

    fn subscribe_protocol_packets(&self) -> crate::udp::VmessUdpFlowResponseReceiver {
        crate::udp::VmessUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message_for_connection(&self) -> &'static str {
        "vmess upstream closed"
    }
}
