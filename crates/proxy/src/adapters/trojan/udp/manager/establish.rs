use super::connect;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    spawn_response_bridge, BoxedManagedStreamUdpConnection, ManagedStreamUdpConnection,
};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use std::sync::Arc;
use zero_core::Session;
use zero_engine::EngineError;

#[async_trait::async_trait]
impl ManagedStreamUdpConnection for trojan::TrojanUdpFlowConnection {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        trojan::TrojanUdpFlowConnection::send(self, target, port, payload)
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(format!("{error}"))))
    }

    fn spawn_response_bridge(
        &self,
        chain_tasks: &mut tokio::task::JoinSet<ChainTask>,
        session_id: u64,
    ) {
        spawn_response_bridge(
            chain_tasks,
            trojan::TrojanUdpFlowConnection::subscribe_responses(self),
            session_id,
            "trojan upstream closed",
            |packet| (packet.target, packet.port, packet.payload),
        );
    }
}

pub(super) async fn direct(
    proxy: &Proxy,
    session: &Session,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<BoxedManagedStreamUdpConnection, EngineError> {
    let tls_stream = connect::direct_tls_stream(proxy, endpoint, resume).await?;

    packet_stream(session, tls_stream, resume).await
}

pub(super) async fn over_relay_stream(
    stream: TcpRelayStream,
    tls_server_name: Option<&str>,
    proxy: &Proxy,
    session: &Session,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<BoxedManagedStreamUdpConnection, EngineError> {
    let tls_stream =
        connect::relay_tls_stream(stream, tls_server_name, proxy, endpoint, resume).await?;

    packet_stream(session, tls_stream, resume).await
}

async fn packet_stream(
    session: &Session,
    stream: TcpRelayStream,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<BoxedManagedStreamUdpConnection, EngineError> {
    let connection = trojan::establish_udp_flow_with_resume(stream, session, resume)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(format!("{error}"))))?;
    Ok(Arc::new(connection))
}
