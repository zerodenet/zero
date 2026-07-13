use std::future::Future;

use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

use crate::managed_udp::{
    ManagedPacketUdpResume, ProtocolManagedStreamUdpBridgeHandlerMetadata,
    ProtocolManagedStreamUdpBridgeOps,
};
use crate::outbound_leaf::{
    ProtocolTcpTransportBridgeMetadata, ProtocolTcpTransportBridgeOps,
    ProtocolUdpTransportBridgeMetadata,
};

use super::leaf::TrojanOutboundLeaf;
use super::managed_udp::TrojanManagedStreamUdpResume;
use super::outbound::TrojanTcpStreamOpen;

#[cfg(feature = "trojan")]
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanTlsBridge;

#[cfg(feature = "trojan")]
impl TrojanTlsBridge {
    pub fn on_config_reloaded(&self) {}
}

impl ProtocolTcpTransportBridgeMetadata for TrojanTlsBridge {
    const TCP_CONNECT_STAGE: &'static str = "connect_upstream_trojan";
    const TCP_INVALID_CONNECT_CONFIG: &'static str = "invalid trojan tcp config";
    const TCP_INVALID_CONNECT_LEAF_STAGE: &'static str = "invalid trojan tcp leaf";
    const TCP_INVALID_RELAY_CONFIG: &'static str = "invalid trojan tcp relay config";
    const TCP_INVALID_RELAY_LEAF_STAGE: &'static str = "invalid trojan tcp relay leaf";
    const EXPECTED_OUTBOUND_LEAF: &'static str = "expected Trojan outbound leaf";
}

impl ProtocolUdpTransportBridgeMetadata for TrojanTlsBridge {
    const UDP_DIRECT_STAGE: &'static str = "udp_trojan_leaf";
    const UDP_INVALID_CONFIG: &'static str = "invalid trojan udp config";
    const UDP_RELAY_FINAL_STAGE: &'static str = "udp_trojan_relay_leaf";
    const EXPECTED_OUTBOUND_LEAF: &'static str = "expected Trojan outbound leaf";
}

#[async_trait::async_trait]
impl<'a> ProtocolTcpTransportBridgeOps<TrojanOutboundLeaf<'a>> for TrojanTlsBridge {
    type Opened = TrojanTcpStreamOpen;

    async fn open_tcp_stream_for_leaf<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        leaf: &TrojanOutboundLeaf<'a>,
        open_socket: OpenSocket,
    ) -> Result<Self::Opened, EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send,
    {
        let _ = self;
        leaf.open_tcp_stream(session, open_socket).await
    }

    async fn open_tcp_relay_hop_for_leaf(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        leaf: &TrojanOutboundLeaf<'a>,
    ) -> Result<TcpRelayStream, EngineError> {
        let _ = self;
        leaf.open_tcp_relay_hop(stream, session).await
    }
}

impl<'a> ProtocolManagedStreamUdpBridgeOps<TrojanOutboundLeaf<'a>> for TrojanTlsBridge {
    type Resume = TrojanManagedStreamUdpResume;

    fn direct_udp_resume_for_leaf(&self, leaf: &TrojanOutboundLeaf<'a>) -> Self::Resume {
        let _ = self;
        ManagedPacketUdpResume::new(leaf.direct_udp_resume())
    }

    fn relay_final_hop_udp_resume_for_leaf(&self, leaf: &TrojanOutboundLeaf<'a>) -> Self::Resume {
        let _ = self;
        ManagedPacketUdpResume::new(leaf.relay_final_hop_udp_resume())
    }
}

impl ProtocolManagedStreamUdpBridgeHandlerMetadata for TrojanTlsBridge {
    type Resume = TrojanManagedStreamUdpResume;
}
