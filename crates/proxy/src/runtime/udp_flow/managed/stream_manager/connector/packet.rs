use async_trait::async_trait;
use zero_core::Session;
use zero_engine::EngineError;
use zero_transport::managed_udp::{
    ManagedPacketUdpResume, ProtocolManagedPacketUdpFlowResumeConnectionOps,
};

use super::super::super::connection::{
    managed_packet_udp_connection_from_flow, SharedManagedUdpConnection,
};
use super::flow::{
    managed_stream_connector_flow_from_build, ManagedStreamConnectorFlow,
    ManagedStreamFlowConnector,
};
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

#[async_trait]
impl<T> ManagedStreamFlowConnector for ManagedPacketUdpResume<T>
where
    T: ProtocolManagedPacketUdpFlowResumeConnectionOps + Clone,
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
        proxy: &Proxy,
        session: &Session,
        _endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let connection = self
            .0
            .open_direct_protocol_connection(session, move |server, port| {
                proxy.connect_upstream_host_owned(server.to_owned(), port)
            })
            .await?;
        Ok(managed_packet_udp_connection_from_flow(connection))
    }

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        _proxy: Option<&Proxy>,
        session: &Session,
        _endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let connection = self
            .0
            .open_relay_protocol_connection(stream, session, tls_server_name)
            .await?;
        Ok(managed_packet_udp_connection_from_flow(connection))
    }
}
