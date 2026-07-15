use std::future::Future;

use zero_core::Session;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};
use zero_transport::managed_udp::{ManagedTupleUdpResume, ProtocolManagedStreamUdpBridgeOps};
use zero_transport::outbound_leaf::ProtocolTcpTransportBridgeOps;
use zero_transport::RuntimeError;

use super::leaf::VmessOutboundLeaf;
use super::managed_udp::VmessManagedStreamUdpResume;

#[derive(Debug, Clone, Default)]
pub struct VmessStreamBridge {
    mux_pool: crate::mux::VmessMuxConnectionPool,
}

impl VmessStreamBridge {
    pub fn on_config_reloaded(&self) {
        self.mux_pool.evict_all();
    }
}

#[async_trait::async_trait]
impl ProtocolTcpTransportBridgeOps<VmessOutboundLeaf> for VmessStreamBridge {
    type Opened = crate::outbound::VmessTcpStreamOpen;

    async fn open_tcp_stream_for_leaf<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        leaf: &VmessOutboundLeaf,
        open_socket: OpenSocket,
    ) -> Result<Self::Opened, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
    {
        leaf.open_tcp_stream(session, &self.mux_pool, open_socket)
            .await
    }

    async fn open_tcp_relay_hop_for_leaf(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        leaf: &VmessOutboundLeaf,
    ) -> Result<TcpRelayStream, RuntimeError> {
        let _ = self;
        leaf.open_tcp_relay_hop(stream, session).await
    }
}

impl ProtocolManagedStreamUdpBridgeOps<VmessOutboundLeaf> for VmessStreamBridge {
    type Resume = VmessManagedStreamUdpResume;

    fn direct_udp_resume_for_leaf(&self, leaf: &VmessOutboundLeaf) -> Self::Resume {
        ManagedTupleUdpResume::new(leaf.direct_udp_resume(self.mux_pool.clone()))
    }

    fn relay_final_hop_udp_resume_for_leaf(&self, leaf: &VmessOutboundLeaf) -> Self::Resume {
        ManagedTupleUdpResume::new(leaf.relay_final_hop_udp_resume(self.mux_pool.clone()))
    }
}
