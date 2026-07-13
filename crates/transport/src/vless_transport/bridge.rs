use std::future::Future;

use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};

use crate::managed_udp::{
    ManagedTupleUdpResume, ProtocolManagedStreamUdpBridgeHandlerMetadata,
    ProtocolManagedStreamUdpBridgeOps, ProtocolRelayTwoStreamManagedUdpBridgeOps,
};
use crate::outbound_leaf::{
    ProtocolRelayTwoStreamUdpTransportBridgeMetadata, ProtocolTcpTransportBridgeMetadata,
    ProtocolTcpTransportBridgeOps, ProtocolUdpTransportBridgeMetadata,
};

use super::leaf::VlessOutboundLeaf;
use super::managed_udp::VlessManagedStreamUdpResume;

#[derive(Debug, Clone)]
pub struct VlessStreamBridge {
    mux_pool: vless::mux_pool::MuxConnectionPool,
}

impl Default for VlessStreamBridge {
    fn default() -> Self {
        Self {
            mux_pool: vless::mux_pool::MuxConnectionPool::new(),
        }
    }
}

impl VlessStreamBridge {
    pub fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }
}

impl ProtocolTcpTransportBridgeMetadata for VlessStreamBridge {
    const TCP_CONNECT_STAGE: &'static str = "connect_upstream_vless";
    const TCP_INVALID_CONNECT_CONFIG: &'static str = "invalid vless tcp config";
    const TCP_INVALID_CONNECT_LEAF_STAGE: &'static str = "invalid vless tcp leaf";
    const TCP_INVALID_RELAY_CONFIG: &'static str = "invalid vless tcp relay config";
    const TCP_INVALID_RELAY_LEAF_STAGE: &'static str = "invalid vless tcp relay leaf";
    const EXPECTED_OUTBOUND_LEAF: &'static str = "expected VLESS outbound leaf";
}

impl ProtocolUdpTransportBridgeMetadata for VlessStreamBridge {
    const UDP_DIRECT_STAGE: &'static str = "udp_vless_leaf";
    const UDP_INVALID_CONFIG: &'static str = "invalid vless udp config";
    const UDP_RELAY_FINAL_STAGE: &'static str = "udp_vless_relay_final_leaf";
    const EXPECTED_OUTBOUND_LEAF: &'static str = "expected VLESS outbound leaf";
}

impl ProtocolRelayTwoStreamUdpTransportBridgeMetadata for VlessStreamBridge {
    const UDP_RELAY_CAPABILITY_STAGE: &'static str = "udp_vless_relay_capability";
    const UDP_RELAY_CHAIN_STAGE: &'static str = "udp_vless_relay_chain";
}

#[async_trait::async_trait]
impl<'a> ProtocolTcpTransportBridgeOps<VlessOutboundLeaf<'a>> for VlessStreamBridge {
    type Opened = vless::outbound::VlessTcpStreamOpen;

    async fn open_tcp_stream_for_leaf<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        leaf: &VlessOutboundLeaf<'a>,
        open_socket: OpenSocket,
    ) -> Result<Self::Opened, EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send,
    {
        leaf.open_tcp_stream(session, &self.mux_pool, open_socket)
            .await
    }

    async fn open_tcp_relay_hop_for_leaf(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        leaf: &VlessOutboundLeaf<'a>,
    ) -> Result<TcpRelayStream, EngineError> {
        let _ = self;
        leaf.open_tcp_relay_hop(stream, session).await
    }
}

impl<'a> ProtocolManagedStreamUdpBridgeOps<VlessOutboundLeaf<'a>> for VlessStreamBridge {
    type Resume = VlessManagedStreamUdpResume;

    fn direct_udp_resume_for_leaf(&self, leaf: &VlessOutboundLeaf<'a>) -> Self::Resume {
        ManagedTupleUdpResume::new(leaf.direct_udp_resume(self.mux_pool.clone()))
    }

    fn relay_final_hop_udp_resume_for_leaf(&self, leaf: &VlessOutboundLeaf<'a>) -> Self::Resume {
        ManagedTupleUdpResume::new(leaf.relay_final_hop_udp_resume(self.mux_pool.clone()))
    }
}

impl ProtocolManagedStreamUdpBridgeHandlerMetadata for VlessStreamBridge {
    type Resume = VlessManagedStreamUdpResume;
}

impl<'a> ProtocolRelayTwoStreamManagedUdpBridgeOps<VlessOutboundLeaf<'a>> for VlessStreamBridge {
    fn udp_relay_needs_two_streams_for_leaf(&self, leaf: &VlessOutboundLeaf<'a>) -> bool {
        let _ = self;
        leaf.relay_needs_two_streams()
    }

    fn relay_two_stream_udp_resume_for_leaf(&self, leaf: &VlessOutboundLeaf<'a>) -> Self::Resume {
        ManagedTupleUdpResume::new(leaf.relay_two_stream_udp_resume(self.mux_pool.clone()))
    }
}
