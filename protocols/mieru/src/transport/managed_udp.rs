use std::future::Future;

use zero_core::Session;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};
use zero_transport::managed_udp::{
    ManagedConnectorFlow, ManagedConnectorFlowOps, ManagedTupleUdpConnectionOps,
    ManagedTupleUdpResume, ProtocolManagedTupleUdpFlowResumeConnectionOps,
    ProtocolManagedTupleUdpResumeMetadata,
};
use zero_transport::RuntimeError;

#[derive(Debug, Clone)]
pub struct MieruManagedUdpFlowResume {
    server: String,
    port: u16,
    protocol: crate::udp::MieruUdpFlowResume,
}

#[derive(Debug, Clone, Copy)]
pub struct MieruManagedUdpFlowConfig<'a> {
    server: &'a str,
    port: u16,
    protocol: crate::udp::MieruUdpFlowConfig<'a>,
}

pub type MieruManagedStreamUdpResume = ManagedTupleUdpResume<MieruManagedUdpFlowResume>;

impl ProtocolManagedTupleUdpResumeMetadata for MieruManagedUdpFlowResume {
    const ESTABLISH_STAGE: &'static str = "mieru_establish";
    const RELAY_UPSTREAM_STAGE: &'static str = "mieru_relay_upstream";
    const RELAY_ESTABLISH_STAGE: &'static str = "mieru_relay_establish";
    const RELAY_SEND_STAGE: &'static str = "mieru_relay_send";
    const MISMATCH_STAGE: &'static str = "udp_mieru_resume";
    const MISMATCH_MESSAGE: &'static str = "expected Mieru UDP flow resume";
}

impl<'a> MieruManagedUdpFlowConfig<'a> {
    pub fn new(server: &'a str, port: u16, username: &'a str, password: &'a str) -> Self {
        Self {
            server,
            port,
            protocol: crate::udp::MieruUdpFlowConfig::new(username, password),
        }
    }

    pub fn flow_resume(&self, relay_chain: bool) -> MieruManagedStreamUdpResume {
        ManagedTupleUdpResume::new(MieruManagedUdpFlowResume::new(
            self.server,
            self.port,
            self.protocol.flow_resume(relay_chain),
        ))
    }
}

impl MieruManagedUdpFlowResume {
    fn new(server: &str, port: u16, protocol: crate::udp::MieruUdpFlowResume) -> Self {
        Self {
            server: server.to_owned(),
            port,
            protocol,
        }
    }

    fn connector_flow(
        &self,
        session_id: u64,
    ) -> ManagedConnectorFlow<crate::udp::MieruUdpConnectorFlow> {
        ManagedConnectorFlow(crate::udp::connector_flow_from_resume(
            &self.protocol,
            &self.server,
            self.port,
            session_id,
        ))
    }

    async fn open_direct_connection<OpenSocket, OpenSocketFut>(
        &self,
        open_socket: OpenSocket,
    ) -> Result<crate::udp::MieruUdpFlowConnection, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        let stream = open_socket(&self.server, self.port).await?;
        crate::udp::establish_udp_flow_with_resume(stream, &self.protocol)
            .await
            .map_err(|error| {
                RuntimeError::Io(std::io::Error::other(format!(
                    "mieru udp associate: {error}"
                )))
            })
    }

    async fn open_relay_connection(
        &self,
        stream: TcpRelayStream,
    ) -> Result<crate::udp::MieruUdpFlowConnection, RuntimeError> {
        crate::udp::establish_udp_flow_with_resume(stream, &self.protocol)
            .await
            .map_err(|error| {
                RuntimeError::Io(std::io::Error::other(format!(
                    "mieru udp associate: {error}"
                )))
            })
    }
}

impl ManagedConnectorFlowOps for crate::udp::MieruUdpConnectorFlow {
    fn into_managed_connector_parts(self) -> (String, bool) {
        crate::udp::MieruUdpConnectorFlow::into_parts(self)
    }
}

#[async_trait::async_trait]
impl ProtocolManagedTupleUdpFlowResumeConnectionOps for MieruManagedUdpFlowResume {
    type ConnectorFlow = ManagedConnectorFlow<crate::udp::MieruUdpConnectorFlow>;
    type RawConnection = crate::udp::MieruUdpFlowConnection;

    fn connector_flow_for_resume(
        &self,
        _server: &str,
        _port: u16,
        session_id: u64,
    ) -> Self::ConnectorFlow {
        MieruManagedUdpFlowResume::connector_flow(self, session_id)
    }

    async fn open_direct_protocol_connection<OpenSocket, OpenSocketFut>(
        &self,
        _session: &Session,
        open_socket: OpenSocket,
    ) -> Result<Self::RawConnection, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        MieruManagedUdpFlowResume::open_direct_connection(self, open_socket).await
    }

    async fn open_relay_protocol_connection(
        &self,
        stream: TcpRelayStream,
        _session: &Session,
        _tls_server_name: Option<&str>,
    ) -> Result<Self::RawConnection, RuntimeError> {
        MieruManagedUdpFlowResume::open_relay_connection(self, stream).await
    }
}

#[async_trait::async_trait]
impl ManagedTupleUdpConnectionOps for crate::udp::MieruUdpFlowConnection {
    type SendError = zero_core::Error;

    async fn send_protocol_packet(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Self::SendError> {
        crate::udp::MieruUdpFlowConnection::send(self, target, port, payload).await
    }

    fn subscribe_protocol_packets(&self) -> crate::udp::MieruUdpFlowResponseReceiver {
        crate::udp::MieruUdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message_for_connection(&self) -> &'static str {
        "mieru upstream closed"
    }
}
