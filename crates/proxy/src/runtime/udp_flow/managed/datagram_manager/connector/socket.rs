use std::marker::PhantomData;

use async_trait::async_trait;
use zero_engine::EngineError;
use zero_transport::managed_udp::ProtocolManagedDatagramSocketUdpResumeConnectionOps;

use super::super::super::connection::SharedManagedDatagramUdpConnection;
use super::super::super::datagram::managed_datagram_connection_from_ops;
use super::super::manager::ManagedDatagramSocketFlowManager;
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::path::OutboundEndpoint;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;

#[async_trait]
pub(crate) trait ManagedDatagramSocketFlowConnector<T>: Send + Sync {
    fn connector_flow(
        &self,
        resume: &T,
        endpoint: OutboundEndpoint,
    ) -> ManagedDatagramSocketConnectorFlow;

    async fn establish(
        &self,
        services: Option<UdpRuntimeServices>,
        endpoint: OutboundEndpoint,
        resume: T,
        initial_packet: UdpPacketRef<'_>,
    ) -> Result<SharedManagedDatagramUdpConnection, EngineError>;
}

pub(crate) struct ManagedDatagramSocketConnectorFlow {
    cache_key: String,
}

impl ManagedDatagramSocketConnectorFlow {
    pub(crate) fn new(cache_key: String) -> Self {
        Self { cache_key }
    }

    pub(crate) fn into_cache_key(self) -> String {
        self.cache_key
    }
}

pub(crate) trait ManagedDatagramSocketConnectorFlowBuild {
    fn into_cache_key(self) -> String;
}

impl ManagedDatagramSocketConnectorFlowBuild for String {
    fn into_cache_key(self) -> String {
        self
    }
}

fn managed_datagram_socket_connector_flow_from_build(
    build: impl ManagedDatagramSocketConnectorFlowBuild,
) -> ManagedDatagramSocketConnectorFlow {
    ManagedDatagramSocketConnectorFlow::new(build.into_cache_key())
}

struct ManagedDatagramSocketResumeConnector<T>(PhantomData<fn() -> T>);

pub(crate) fn managed_datagram_socket_handler_box<T>(
) -> Box<dyn super::super::super::model::ManagedDatagramFlowHandler>
where
    T: ProtocolManagedDatagramSocketUdpResumeConnectionOps,
{
    Box::new(ManagedDatagramSocketFlowManager::new(
        ManagedDatagramSocketResumeConnector::<T>(PhantomData),
        T::ESTABLISH_STAGE,
        T::SEND_STAGE,
        T::MISMATCH_STAGE,
        T::MISMATCH_MESSAGE,
    ))
}

#[async_trait]
impl<T> ManagedDatagramSocketFlowConnector<T> for ManagedDatagramSocketResumeConnector<T>
where
    T: ProtocolManagedDatagramSocketUdpResumeConnectionOps,
{
    fn connector_flow(
        &self,
        resume: &T,
        endpoint: OutboundEndpoint,
    ) -> ManagedDatagramSocketConnectorFlow {
        managed_datagram_socket_connector_flow_from_build(
            resume.connector_flow_cache_key(&endpoint.server, endpoint.port),
        )
    }

    async fn establish(
        &self,
        services: Option<UdpRuntimeServices>,
        endpoint: OutboundEndpoint,
        resume: T,
        _initial_packet: UdpPacketRef<'_>,
    ) -> Result<SharedManagedDatagramUdpConnection, EngineError> {
        let services = services
            .ok_or_else(|| EngineError::Io(std::io::Error::other(T::PROXY_CONTEXT_MESSAGE)))?;
        let target_addr = services
            .resolve_direct_address(
                &endpoint.address(),
                endpoint.port,
                T::RESOLVE_UPSTREAM_MESSAGE,
            )
            .await?;
        let connection = resume.open_protocol_connection(target_addr).await?;
        Ok(managed_datagram_connection_from_ops(connection))
    }
}
