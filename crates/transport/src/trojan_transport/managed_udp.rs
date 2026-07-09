use std::future::Future;

use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

use crate::managed_udp::{
    ManagedConnectorFlow, ManagedConnectorFlowOps, ManagedPacketUdpConnectionOps,
    ManagedPacketUdpResume, ProtocolManagedPacketUdpFlowResumeConnectionOps,
    ProtocolManagedStreamUdpResumeMetadata,
};

use super::outbound::OwnedTrojanOutboundTlsPlan;

#[cfg(feature = "trojan")]
#[derive(Debug, Clone)]
pub struct TrojanManagedUdpFlowResume {
    transport: OwnedTrojanOutboundTlsPlan,
    protocol: trojan::udp::PreparedTrojanUdpFlowPlan,
}

type TrojanManagedUdpConnectorFlow = ManagedConnectorFlow<trojan::udp::TrojanUdpConnectorFlow>;

pub type TrojanManagedStreamUdpResume = ManagedPacketUdpResume<TrojanManagedUdpFlowResume>;

impl ProtocolManagedStreamUdpResumeMetadata for TrojanManagedStreamUdpResume {
    const ESTABLISH_STAGE: &'static str = "trojan_establish";
    const RELAY_UPSTREAM_STAGE: &'static str = "trojan_relay_upstream";
    const RELAY_ESTABLISH_STAGE: &'static str = "trojan_relay_establish";
    const RELAY_SEND_STAGE: &'static str = "trojan_relay_send";
    const MISMATCH_STAGE: &'static str = "udp_trojan_resume";
    const MISMATCH_MESSAGE: &'static str = "expected Trojan UDP flow resume";
}

#[cfg(feature = "trojan")]
impl TrojanManagedUdpFlowResume {
    pub(super) fn new(
        transport: OwnedTrojanOutboundTlsPlan,
        protocol: trojan::udp::PreparedTrojanUdpFlowPlan,
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
    ) -> Result<trojan::udp::TrojanUdpFlowConnection, EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send,
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
    ) -> Result<trojan::udp::TrojanUdpFlowConnection, EngineError> {
        let transport = self.transport.clone();
        self.protocol
            .open_udp_flow_with_transport(session, tls_server_name, move |tls_profile| async move {
                transport.open_relay_with_profile(stream, tls_profile).await
            })
            .await
    }
}

impl ManagedConnectorFlowOps for trojan::udp::TrojanUdpConnectorFlow {
    fn into_managed_connector_parts(self) -> (String, bool) {
        trojan::udp::TrojanUdpConnectorFlow::into_parts(self)
    }
}

#[async_trait::async_trait]
impl ProtocolManagedPacketUdpFlowResumeConnectionOps for TrojanManagedUdpFlowResume {
    type ConnectorFlow = TrojanManagedUdpConnectorFlow;
    type RawConnection = trojan::udp::TrojanUdpFlowConnection;

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
    ) -> Result<Self::RawConnection, EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send,
    {
        TrojanManagedUdpFlowResume::open_direct_connection(self, session, open_socket).await
    }

    async fn open_relay_protocol_connection(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        tls_server_name: Option<&str>,
    ) -> Result<Self::RawConnection, EngineError> {
        TrojanManagedUdpFlowResume::open_relay_connection(self, stream, session, tls_server_name)
            .await
    }
}

#[async_trait::async_trait]
impl ManagedPacketUdpConnectionOps for trojan::udp::TrojanUdpFlowConnection {
    type SendError = zero_core::Error;

    async fn send_protocol_packet(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Self::SendError> {
        trojan::udp::TrojanUdpFlowConnection::send(self, target, port, payload).await
    }

    fn subscribe_protocol_packets(&self) -> trojan::udp::TrojanUdpFlowResponseReceiver {
        trojan::udp::TrojanUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message_for_connection(&self) -> &'static str {
        "trojan upstream closed"
    }
}
