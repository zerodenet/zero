use std::future::Future;

use zero_core::Session;
use zero_platform_tokio::TokioSocket;
use zero_transport::RuntimeError;
use zero_transport::TcpRelayStream;

use super::outbound::OwnedTrojanOutboundTlsPlan;

#[derive(Debug, Clone)]
pub struct TrojanManagedUdpFlowResume {
    transport: OwnedTrojanOutboundTlsPlan,
    protocol: crate::udp::PreparedTrojanUdpFlowPlan,
}

pub type TrojanManagedUdpConnectorFlow = crate::udp::TrojanUdpConnectorFlow;

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

    pub fn connector_flow(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> TrojanManagedUdpConnectorFlow {
        self.protocol.connector_flow(server, port, session_id)
    }

    pub async fn open_direct_connection<OpenSocket, OpenSocketFut>(
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

    pub async fn open_relay_connection(
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
