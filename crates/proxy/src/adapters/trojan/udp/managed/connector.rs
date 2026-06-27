use std::sync::Arc;

use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    managed_packet_udp_connection, ManagedPacketUdpSender, ManagedStreamFlowConnector,
    SharedManagedUdpConnection,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_core::Session;
use zero_engine::EngineError;

pub(super) struct TrojanManagedStreamConnector;

#[async_trait::async_trait]
impl ManagedStreamFlowConnector<trojan::TrojanUdpFlowResume> for TrojanManagedStreamConnector {
    fn flow_cache_key(
        &self,
        resume: &trojan::TrojanUdpFlowResume,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> String {
        resume.flow_cache_key(endpoint.server, endpoint.port, session_id)
    }

    fn requires_relay_upstream(&self, resume: &trojan::TrojanUdpFlowResume) -> bool {
        resume.flow_requires_relay_upstream()
    }

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
        resume: trojan::TrojanUdpFlowResume,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let tls_stream =
            crate::outbound::trojan::open_udp_tls_stream(proxy, endpoint, &resume).await?;
        packet_stream(session, tls_stream, resume).await
    }

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        proxy: Option<&Proxy>,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
        resume: trojan::TrojanUdpFlowResume,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let proxy = proxy.ok_or_else(|| {
            EngineError::Io(std::io::Error::other(
                "expected proxy context for Trojan UDP relay flow",
            ))
        })?;
        let tls_stream = crate::outbound::trojan::open_udp_tls_relay_stream(
            stream,
            tls_server_name,
            proxy,
            endpoint,
            &resume,
        )
        .await?;
        packet_stream(session, tls_stream, resume).await
    }
}

async fn packet_stream(
    session: &Session,
    stream: TcpRelayStream,
    resume: trojan::TrojanUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let connection = trojan::establish_udp_flow_with_resume(stream, session, &resume)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(format!("{error}"))))?;
    Ok(managed_packet_udp_connection(Arc::new(
        TrojanManagedUdpSender { connection },
    )))
}

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
