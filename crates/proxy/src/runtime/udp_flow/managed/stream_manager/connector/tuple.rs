use async_trait::async_trait;
use zero_core::Session;
use zero_engine::EngineError;
use zero_transport::managed_udp::{
    ManagedTupleUdpResume, ProtocolManagedTupleUdpFlowResumeConnectionOps,
};

use super::super::super::connection::{
    managed_tuple_udp_connection_from_ops, SharedManagedUdpConnection,
};
use super::flow::{
    managed_stream_connector_flow_from_build, ManagedStreamConnectorFlow,
    ManagedStreamFlowConnector,
};
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::path::OutboundEndpoint;
use crate::transport::TcpRelayStream;

#[async_trait]
impl<T> ManagedStreamFlowConnector for ManagedTupleUdpResume<T>
where
    T: ProtocolManagedTupleUdpFlowResumeConnectionOps + Clone,
{
    fn connector_flow(
        &self,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow {
        managed_stream_connector_flow_from_build(self.0.connector_flow_for_resume(
            endpoint.server,
            endpoint.port,
            session_id,
        ))
    }

    async fn establish_direct(
        &self,
        services: UdpRuntimeServices,
        session: &Session,
        _endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let connection = self
            .0
            .open_direct_protocol_connection(session, move |server, port| {
                let services = services.clone();
                let server = server.to_owned();
                async move { services.connect_upstream(&server, port).await }
            })
            .await?;
        Ok(managed_tuple_udp_connection_from_ops(connection))
    }

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        _services: Option<UdpRuntimeServices>,
        session: &Session,
        _endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let connection = self
            .0
            .open_relay_protocol_connection(stream, session, tls_server_name)
            .await?;
        Ok(managed_tuple_udp_connection_from_ops(connection))
    }
}
