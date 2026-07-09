use std::future::Future;

use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

use crate::outbound_leaf::{ProtocolTcpTransportOpenResult, ProtocolTransportLeaf};
use crate::StreamTraffic;

use super::managed_udp::TrojanManagedUdpFlowResume;
use super::outbound::{OwnedTrojanOutboundTlsPlan, TrojanTcpStreamOpen};

#[derive(Clone)]
pub struct TrojanOutboundLeaf<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    transport: OwnedTrojanOutboundTlsPlan,
    protocol: trojan::outbound::PreparedTrojanOutboundRequestBundle,
}

impl<'a> TrojanOutboundLeaf<'a> {
    pub(super) fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        transport: OwnedTrojanOutboundTlsPlan,
        protocol: trojan::outbound::PreparedTrojanOutboundRequestBundle,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            protocol,
            transport,
        }
    }

    fn owned_transport_plan(&self) -> OwnedTrojanOutboundTlsPlan {
        self.transport.clone()
    }

    pub(super) async fn open_tcp_stream<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<TrojanTcpStreamOpen, EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send,
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

    pub(super) async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, EngineError> {
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

impl ProtocolTransportLeaf for TrojanOutboundLeaf<'_> {
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

impl ProtocolTcpTransportOpenResult for TrojanTcpStreamOpen {
    fn into_proxied_stream_parts(self) -> (TcpRelayStream, StreamTraffic) {
        let (upstream, handshake_written_bytes) = self.into_parts();
        (
            upstream,
            StreamTraffic {
                read_bytes: 0,
                written_bytes: handshake_written_bytes,
            },
        )
    }
}
