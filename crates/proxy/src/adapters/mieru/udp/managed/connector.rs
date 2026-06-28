use std::sync::Arc;

use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::udp_flow::managed::{
    managed_tuple_udp_connection, ManagedStreamConnectorFlow, ManagedStreamFlowConnector,
    ManagedTupleUdpSender, SharedManagedUdpConnection,
};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use zero_core::Session;
use zero_engine::EngineError;

pub(super) struct MieruManagedStreamConnector;

#[async_trait::async_trait]
impl ManagedStreamFlowConnector<mieru::MieruUdpFlowResume> for MieruManagedStreamConnector {
    fn connector_flow(
        &self,
        resume: &mieru::MieruUdpFlowResume,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow {
        let flow = resume.connector_flow(endpoint.server, endpoint.port, session_id);
        ManagedStreamConnectorFlow::new(flow.cache_key(), flow.requires_relay_upstream())
    }

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        _session: &Session,
        endpoint: OutboundEndpoint<'_>,
        resume: mieru::MieruUdpFlowResume,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let socket = proxy
            .protocols
            .direct_connector()
            .connect_host(endpoint.server, endpoint.port, proxy.resolver.as_ref())
            .await?;
        packet_stream(TcpRelayStream::new(socket), resume).await
    }

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        _tls_server_name: Option<&str>,
        _proxy: Option<&Proxy>,
        _session: &Session,
        _endpoint: OutboundEndpoint<'_>,
        resume: mieru::MieruUdpFlowResume,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        packet_stream(stream, resume).await
    }
}

async fn packet_stream(
    stream: TcpRelayStream,
    resume: mieru::MieruUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let connection = mieru::establish_udp_flow_with_resume(stream, &resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;
    Ok(managed_tuple_udp_connection(Arc::new(
        MieruManagedUdpSender { connection },
    )))
}

struct MieruManagedUdpSender {
    connection: mieru::MieruUdpFlowConnection,
}

#[async_trait::async_trait]
impl ManagedTupleUdpSender for MieruManagedUdpSender {
    async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.connection
            .send(target, port, payload)
            .await
            .map_err(|error| {
                EngineError::Io(std::io::Error::other(format!("mieru udp send: {error}")))
            })
    }

    fn subscribe_responses(&self) -> mieru::MieruUdpFlowResponseReceiver {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        "mieru upstream closed"
    }
}
