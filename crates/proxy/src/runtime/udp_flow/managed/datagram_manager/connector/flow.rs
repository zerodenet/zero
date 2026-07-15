use std::marker::PhantomData;

use async_trait::async_trait;
use zero_engine::EngineError;
use zero_transport::managed_udp::ProtocolManagedDatagramUdpResumeConnectionOps;

use super::super::super::connection::{
    managed_tuple_udp_connection_from_ops, SharedManagedUdpConnection,
};
use super::super::manager::ManagedDatagramFlowManager;
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::path::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;

#[async_trait]
pub(crate) trait ManagedDatagramFlowConnector<T>: Send + Sync {
    const INITIAL_PACKET_PRE_SENT: bool;

    fn connector_flow(
        &self,
        resume: &T,
        endpoint: OutboundEndpoint,
    ) -> ManagedDatagramConnectorFlow;

    async fn establish(
        &self,
        services: Option<UdpRuntimeServices>,
        endpoint: OutboundEndpoint,
        resume: T,
        initial_packet: UdpPacketRef<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError>;
}

pub(crate) struct ManagedDatagramConnectorFlow {
    cache_key: String,
}

impl ManagedDatagramConnectorFlow {
    pub(crate) fn new(cache_key: String) -> Self {
        Self { cache_key }
    }

    pub(crate) fn into_cache_key(self) -> String {
        self.cache_key
    }
}

struct ManagedDatagramResumeConnector<T>(PhantomData<fn() -> T>);

pub(crate) fn managed_datagram_handler_box<T>(
) -> Box<dyn super::super::super::model::ManagedDatagramFlowHandler>
where
    T: ProtocolManagedDatagramUdpResumeConnectionOps,
{
    Box::new(ManagedDatagramFlowManager::new(
        ManagedDatagramResumeConnector::<T>(PhantomData),
        T::ESTABLISH_STAGE,
        T::MISMATCH_STAGE,
        T::MISMATCH_MESSAGE,
    ))
}

#[async_trait]
impl<T> ManagedDatagramFlowConnector<T> for ManagedDatagramResumeConnector<T>
where
    T: ProtocolManagedDatagramUdpResumeConnectionOps,
{
    const INITIAL_PACKET_PRE_SENT: bool = true;

    fn connector_flow(
        &self,
        resume: &T,
        endpoint: OutboundEndpoint,
    ) -> ManagedDatagramConnectorFlow {
        ManagedDatagramConnectorFlow::new(
            resume.connector_flow_cache_key(&endpoint.server, endpoint.port),
        )
    }

    async fn establish(
        &self,
        _services: Option<UdpRuntimeServices>,
        endpoint: OutboundEndpoint,
        resume: T,
        initial_packet: UdpPacketRef<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError> {
        let connection = resume
            .open_protocol_connection(
                &endpoint.server,
                endpoint.port,
                initial_packet.target,
                initial_packet.port,
                initial_packet.payload,
            )
            .await?;
        Ok(managed_tuple_udp_connection_from_ops(connection))
    }
}
