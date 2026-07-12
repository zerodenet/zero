use std::any::Any;

use async_trait::async_trait;
use zero_core::Session;
use zero_engine::EngineError;
use zero_transport::managed_udp::ProtocolManagedStreamConnectorParts;

use super::super::super::connection::SharedManagedUdpConnection;
use crate::runtime::orchestration::OutboundEndpoint;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

#[async_trait]
pub(crate) trait ManagedStreamFlowConnector:
    Any + Clone + Send + Sync + std::fmt::Debug + 'static
{
    fn connector_flow(
        &self,
        endpoint: OutboundEndpoint<'_>,
        session_id: u64,
    ) -> ManagedStreamConnectorFlow;

    async fn establish_direct(
        &self,
        proxy: &Proxy,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError>;

    async fn establish_relay(
        &self,
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        proxy: Option<&Proxy>,
        session: &Session,
        endpoint: OutboundEndpoint<'_>,
    ) -> Result<SharedManagedUdpConnection, EngineError>;
}

pub(crate) struct ManagedStreamConnectorFlow {
    cache_key: String,
    requires_relay_upstream: bool,
}

impl ManagedStreamConnectorFlow {
    pub(crate) fn new(cache_key: String, requires_relay_upstream: bool) -> Self {
        Self {
            cache_key,
            requires_relay_upstream,
        }
    }

    pub(crate) fn into_parts(self) -> (String, bool) {
        (self.cache_key, self.requires_relay_upstream)
    }
}

pub(crate) trait ManagedStreamConnectorFlowBuild {
    fn into_parts(self) -> (String, bool);
}

impl<T> ManagedStreamConnectorFlowBuild for T
where
    T: ProtocolManagedStreamConnectorParts,
{
    fn into_parts(self) -> (String, bool) {
        self.into_managed_connector_parts()
    }
}

pub(crate) fn managed_stream_connector_flow_from_build(
    build: impl ManagedStreamConnectorFlowBuild,
) -> ManagedStreamConnectorFlow {
    let (cache_key, requires_relay_upstream) = build.into_parts();
    ManagedStreamConnectorFlow::new(cache_key, requires_relay_upstream)
}
