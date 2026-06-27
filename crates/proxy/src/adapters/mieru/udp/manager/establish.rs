use super::connect;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    spawn_tuple_response_bridge, ManagedUdpConnection, SharedManagedUdpConnection,
};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use std::sync::Arc;
use zero_engine::EngineError;

#[async_trait::async_trait]
impl ManagedUdpConnection for mieru::MieruUdpFlowConnection {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        mieru::MieruUdpFlowConnection::send(self, target, port, payload)
            .await
            .map_err(|error| {
                EngineError::Io(std::io::Error::other(format!("mieru udp send: {error}")))
            })
    }

    fn spawn_response_bridge(
        &self,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        session_id: u64,
    ) {
        spawn_tuple_response_bridge(
            chain_tasks,
            mieru::MieruUdpFlowConnection::subscribe_responses(self),
            session_id,
            "mieru upstream closed",
        );
    }
}

pub(super) async fn direct(
    proxy: &Proxy,
    endpoint: OutboundEndpoint<'_>,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let stream = connect::direct_stream(proxy, endpoint).await?;
    packet_stream(stream, resume).await
}

pub(super) async fn packet_stream(
    stream: TcpRelayStream,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let connection = mieru::establish_udp_flow_with_resume(stream, resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;
    Ok(Arc::new(connection))
}
