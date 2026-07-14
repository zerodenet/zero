use std::future::Future;

use zero_core::Session;
use zero_platform_tokio::TokioSocket;
use zero_transport::managed_udp::{
    ManagedConnectorFlow, ManagedConnectorFlowOps, ManagedPacketUdpConnectionOps,
    ManagedPacketUdpResume, ProtocolManagedPacketUdpFlowResumeConnectionOps,
    ProtocolManagedPacketUdpResumeMetadata,
};
use zero_transport::RuntimeError;
use zero_transport::TcpRelayStream;

use super::outbound::OwnedTrojanOutboundTlsPlan;

#[derive(Debug, Clone)]
pub struct TrojanManagedUdpFlowResume {
    transport: OwnedTrojanOutboundTlsPlan,
    protocol: crate::udp::PreparedTrojanUdpFlowPlan,
}

type TrojanManagedUdpConnectorFlow = ManagedConnectorFlow<crate::udp::TrojanUdpConnectorFlow>;

pub type TrojanManagedStreamUdpResume = ManagedPacketUdpResume<TrojanManagedUdpFlowResume>;

impl ProtocolManagedPacketUdpResumeMetadata for TrojanManagedUdpFlowResume {
    const ESTABLISH_STAGE: &'static str = "trojan_establish";
    const RELAY_UPSTREAM_STAGE: &'static str = "trojan_relay_upstream";
    const RELAY_ESTABLISH_STAGE: &'static str = "trojan_relay_establish";
    const RELAY_SEND_STAGE: &'static str = "trojan_relay_send";
    const MISMATCH_STAGE: &'static str = "udp_trojan_resume";
    const MISMATCH_MESSAGE: &'static str = "expected Trojan UDP flow resume";
}

impl TrojanManagedUdpFlowResume {
    pub(super) fn new(
        transport: OwnedTrojanOutboundTlsPlan,
        protocol: crate::udp::PreparedTrojanUdpFlowPlan,
    ) -> Self {
        Self {
            transport,
            protocol,
        }
    }

    fn connector_flow(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> TrojanManagedUdpConnectorFlow {
        ManagedConnectorFlow(self.protocol.connector_flow(server, port, session_id))
    }

    async fn open_direct_connection<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<crate::udp::TrojanUdpFlowConnection, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        let transport = self.transport.clone();
        self.protocol
            .open_udp_flow_with_transport(session, None, move |tls_profile| async move {
                transport
                    .open_direct_with_profile(open_socket, tls_profile)
                    .await
            })
            .await
    }

    async fn open_relay_connection(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        tls_server_name: Option<&str>,
    ) -> Result<crate::udp::TrojanUdpFlowConnection, RuntimeError> {
        let transport = self.transport.clone();
        self.protocol
            .open_udp_flow_with_transport(session, tls_server_name, move |tls_profile| async move {
                transport.open_relay_with_profile(stream, tls_profile).await
            })
            .await
    }
}

impl ManagedConnectorFlowOps for crate::udp::TrojanUdpConnectorFlow {
    fn into_managed_connector_parts(self) -> (String, bool) {
        crate::udp::TrojanUdpConnectorFlow::into_parts(self)
    }
}

#[async_trait::async_trait]
impl ProtocolManagedPacketUdpFlowResumeConnectionOps for TrojanManagedUdpFlowResume {
    type ConnectorFlow = TrojanManagedUdpConnectorFlow;
    type RawConnection = crate::udp::TrojanUdpFlowConnection;

    fn connector_flow_for_resume(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Self::ConnectorFlow {
        TrojanManagedUdpFlowResume::connector_flow(self, server, port, session_id)
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
        TrojanManagedUdpFlowResume::open_direct_connection(self, session, open_socket).await
    }

    async fn open_relay_protocol_connection(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        tls_server_name: Option<&str>,
    ) -> Result<Self::RawConnection, RuntimeError> {
        TrojanManagedUdpFlowResume::open_relay_connection(self, stream, session, tls_server_name)
            .await
    }
}

#[async_trait::async_trait]
impl ManagedPacketUdpConnectionOps for crate::udp::TrojanUdpFlowConnection {
    type SendError = zero_core::Error;

    async fn send_protocol_packet(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Self::SendError> {
        crate::udp::TrojanUdpFlowConnection::send(self, target, port, payload).await
    }

    fn subscribe_protocol_packets(&self) -> crate::udp::TrojanUdpFlowResponseReceiver {
        crate::udp::TrojanUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message_for_connection(&self) -> &'static str {
        "trojan upstream closed"
    }
}
