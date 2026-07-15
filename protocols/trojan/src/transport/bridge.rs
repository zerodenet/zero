use std::future::Future;

use zero_core::Session;
use zero_platform_tokio::TokioSocket;
use zero_transport::managed_udp::{ManagedPacketUdpResume, ProtocolManagedStreamUdpBridgeOps};
use zero_transport::outbound_leaf::ProtocolTcpTransportBridgeOps;
use zero_transport::RuntimeError;
use zero_transport::TcpRelayStream;

use super::leaf::TrojanOutboundLeaf;
use super::managed_udp::TrojanManagedStreamUdpResume;
use super::outbound::TrojanTcpStreamOpen;

#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanTlsBridge;

impl TrojanTlsBridge {
    pub fn on_config_reloaded(&self) {}
}

#[async_trait::async_trait]
impl ProtocolTcpTransportBridgeOps<TrojanOutboundLeaf> for TrojanTlsBridge {
    type Opened = TrojanTcpStreamOpen;

    async fn open_tcp_stream_for_leaf<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        leaf: &TrojanOutboundLeaf,
        open_socket: OpenSocket,
    ) -> Result<Self::Opened, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        let _ = self;
        leaf.open_tcp_stream(session, open_socket).await
    }

    async fn open_tcp_relay_hop_for_leaf(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        leaf: &TrojanOutboundLeaf,
    ) -> Result<TcpRelayStream, RuntimeError> {
        let _ = self;
        leaf.open_tcp_relay_hop(stream, session).await
    }
}

impl ProtocolManagedStreamUdpBridgeOps<TrojanOutboundLeaf> for TrojanTlsBridge {
    type Resume = TrojanManagedStreamUdpResume;

    fn direct_udp_resume_for_leaf(&self, leaf: &TrojanOutboundLeaf) -> Self::Resume {
        let _ = self;
        ManagedPacketUdpResume::new(leaf.direct_udp_resume())
    }

    fn relay_final_hop_udp_resume_for_leaf(&self, leaf: &TrojanOutboundLeaf) -> Self::Resume {
        let _ = self;
        ManagedPacketUdpResume::new(leaf.relay_final_hop_udp_resume())
    }
}
