use super::connect;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    managed_packet_udp_connection, ManagedPacketUdpSender, SharedManagedUdpConnection,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use std::sync::Arc;
use zero_core::Session;
use zero_engine::EngineError;

struct TrojanManagedUdpSender {
    connection: trojan::TrojanUdpFlowConnection,
}

#[async_trait::async_trait]
impl ManagedPacketUdpSender for TrojanManagedUdpSender {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.connection
            .send(target, port, payload)
            .await
            .map_err(|error| EngineError::Io(std::io::Error::other(format!("{error}"))))
    }

    fn subscribe_responses(&self) -> trojan::TrojanUdpFlowResponseReceiver {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        "trojan upstream closed"
    }
}

pub(super) async fn direct(
    proxy: &Proxy,
    session: &Session,
    endpoint: OutboundEndpoint<'_>,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
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
) -> Result<SharedManagedUdpConnection, EngineError> {
    let tls_stream =
        connect::relay_tls_stream(stream, tls_server_name, proxy, endpoint, resume).await?;

    packet_stream(session, tls_stream, resume).await
}

async fn packet_stream(
    session: &Session,
    stream: TcpRelayStream,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let connection = trojan::establish_udp_flow_with_resume(stream, session, resume)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(format!("{error}"))))?;
    Ok(managed_packet_udp_connection(Arc::new(
        TrojanManagedUdpSender { connection },
    )))
}
