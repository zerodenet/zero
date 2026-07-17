use std::future::Future;
use std::path::Path;

use zero_core::Session;
use zero_platform_tokio::TokioSocket;
use zero_traits::ProtocolUdpFlowLeaf;
use zero_transport::RuntimeError;
use zero_transport::TcpRelayStream;

use super::managed_udp::TrojanManagedUdpFlowResume;
use super::options::{TrojanOutboundBuildOptionsRef, TrojanOutboundOptionsRef};
use super::outbound::{OwnedTrojanOutboundTlsPlan, TrojanTcpStreamOpen};

#[derive(Clone)]
pub struct TrojanOutboundLeaf {
    tag: String,
    server: String,
    port: u16,
    transport: OwnedTrojanOutboundTlsPlan,
    protocol: crate::outbound::PreparedTrojanOutboundRequestBundle,
}

impl TrojanOutboundLeaf {
    #[allow(clippy::too_many_arguments)]
    fn from_parts(
        source_dir: Option<&Path>,
        tag: &str,
        server: &str,
        port: u16,
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Self {
        let protocol = crate::outbound::PreparedTrojanOutboundRequestBundle::from_config(
            password,
            sni,
            insecure,
            client_fingerprint,
        );
        let transport = OwnedTrojanOutboundTlsPlan::from_parts(source_dir, server, port);
        Self::new(tag, server, port, transport, protocol)
    }

    pub fn from_options_refs(
        source_dir: Option<&Path>,
        options: TrojanOutboundBuildOptionsRef<'_>,
    ) -> Self {
        let TrojanOutboundBuildOptionsRef {
            tag,
            server,
            port,
            protocol:
                TrojanOutboundOptionsRef {
                    password,
                    sni,
                    insecure,
                    client_fingerprint,
                },
        } = options;
        Self::from_parts(
            source_dir,
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        )
    }

    pub(super) fn new(
        tag: &str,
        server: &str,
        port: u16,
        transport: OwnedTrojanOutboundTlsPlan,
        protocol: crate::outbound::PreparedTrojanOutboundRequestBundle,
    ) -> Self {
        Self {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            protocol,
            transport,
        }
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    fn owned_transport_plan(&self) -> OwnedTrojanOutboundTlsPlan {
        self.transport.clone()
    }

    pub async fn open_tcp_stream<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<TrojanTcpStreamOpen, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        let protocol = self.protocol.clone();
        let transport = self.owned_transport_plan();
        protocol
            .open_tcp_stream_with_transport(session, move |tls_profile| async move {
                transport
                    .open_direct_with_profile(open_socket, tls_profile)
                    .await
            })
            .await
    }

    pub async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError> {
        let protocol = self.protocol.clone();
        let transport = self.owned_transport_plan();
        protocol
            .open_tcp_stream_with_transport(session, move |tls_profile| async move {
                transport.open_relay_with_profile(stream, tls_profile).await
            })
            .await
            .map(|opened| opened.into_parts().0)
    }

    pub(super) fn direct_udp_resume(&self) -> TrojanManagedUdpFlowResume {
        TrojanManagedUdpFlowResume::new(
            self.owned_transport_plan(),
            self.protocol.udp_direct_flow_plan(),
        )
    }

    pub(super) fn relay_final_hop_udp_resume(&self) -> TrojanManagedUdpFlowResume {
        TrojanManagedUdpFlowResume::new(
            self.owned_transport_plan(),
            self.protocol.udp_relay_flow_plan(),
        )
    }
}

impl ProtocolUdpFlowLeaf for TrojanOutboundLeaf {
    type Resume = TrojanManagedUdpFlowResume;

    fn direct_udp_resume(&self) -> Self::Resume {
        TrojanOutboundLeaf::direct_udp_resume(self)
    }

    fn relay_final_hop_udp_resume(&self) -> Self::Resume {
        TrojanOutboundLeaf::relay_final_hop_udp_resume(self)
    }
}
