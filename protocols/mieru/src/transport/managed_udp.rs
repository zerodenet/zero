use std::future::Future;

use zero_platform_tokio::{TcpRelayStream, TokioSocket};
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

pub type MieruManagedUdpConnectorFlow = crate::udp::MieruUdpConnectorFlow;

impl<'a> MieruManagedUdpFlowConfig<'a> {
    pub fn new(server: &'a str, port: u16, username: &'a str, password: &'a str) -> Self {
        Self {
            server,
            port,
            protocol: crate::udp::MieruUdpFlowConfig::new(username, password),
        }
    }

    pub fn flow_resume(&self, relay_chain: bool) -> MieruManagedUdpFlowResume {
        MieruManagedUdpFlowResume::new(
            self.server,
            self.port,
            self.protocol.flow_resume(relay_chain),
        )
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

    pub fn connector_flow(&self, session_id: u64) -> MieruManagedUdpConnectorFlow {
        crate::udp::connector_flow_from_resume(&self.protocol, &self.server, self.port, session_id)
    }

    pub async fn open_direct_connection<OpenSocket, OpenSocketFut>(
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

    pub async fn open_relay_connection(
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
