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

#[async_trait::async_trait]
impl ManagedStreamFlowConnector for mieru::udp::MieruUdpFlowResume {
    fn connector_flow(
        &self,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow {
        let flow = mieru::udp::connector_flow_from_resume(
            self,
            endpoint.server,
            endpoint.port,
            session_id,
        );
        let (cache_key, requires_relay_upstream) =
            mieru::udp::MieruUdpConnectorFlow::into_parts(flow);
        ManagedStreamConnectorFlow::new(cache_key, requires_relay_upstream)
    }

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        _session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let socket = proxy
            .protocols
            .direct_connector()
            .connect_host(endpoint.server, endpoint.port, proxy.resolver.as_ref())
            .await?;
        packet_stream(TcpRelayStream::new(socket), self).await
    }

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        _tls_server_name: Option<&str>,
        _proxy: Option<&Proxy>,
        _session: &Session,
        _endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        packet_stream(stream, self).await
    }
}

async fn packet_stream(
    stream: TcpRelayStream,
    resume: &mieru::udp::MieruUdpFlowResume,
) -> Result<SharedManagedUdpConnection, EngineError> {
    let connection = mieru::udp::establish_udp_flow_with_resume(stream, resume)
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
    connection: mieru::udp::MieruUdpFlowConnection,
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

    fn subscribe_responses(&self) -> mieru::udp::MieruUdpFlowResponseReceiver {
        self.connection.subscribe_responses()
    }

    fn closed_message(&self) -> &'static str {
        "mieru upstream closed"
    }
}
