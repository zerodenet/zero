use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    spawn_tuple_response_bridge, ManagedUdpConnection, SharedManagedUdpConnection,
};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use std::sync::Arc;
use tokio::task::JoinSet;
use zero_engine::EngineError;

#[async_trait::async_trait]
impl ManagedUdpConnection for hysteria2::Hysteria2UdpFlowConnection {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        hysteria2::Hysteria2UdpFlowConnection::send(self, target, port, payload)
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(format!("{error}"))))
    }

    fn spawn_response_bridge(&self, chain_tasks: &mut JoinSet<ChainTask>, session_id: u64) {
        spawn_tuple_response_bridge(
            chain_tasks,
            hysteria2::Hysteria2UdpFlowConnection::subscribe_responses(self),
            session_id,
            "h2 upstream closed",
        );
    }
}

pub(super) async fn upstream(
    endpoint: OutboundEndpoint<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
    initial_packet: UdpPacketRef<'_>,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let session =
        crate::outbound::hysteria2::establish_udp_flow_session(endpoint, initial_packet, resume)
            .await?;

    Ok(Arc::new(session))
}
