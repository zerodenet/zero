use std::future::Future;
use std::pin::Pin;

use zero_core::Session;
use zero_engine::EngineError;
use zero_transport::outbound_leaf::{
    open_prepared_tcp_transport_leaf_relay_hop, open_prepared_tcp_transport_leaf_stream,
    PreparedTransportLeaf, ProtocolTcpTransportLeafMetadata, ProtocolTcpTransportLeafOps,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
};

use super::contract::{PreparedTcpConnectOperation, PreparedTcpRelayOperation};
use crate::protocol_registry::TcpRuntimeServices;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

pub(crate) struct TransportLeafTcpConnectOperation<TLeaf> {
    pub(crate) prepared: PreparedTransportLeaf<TLeaf>,
}

impl<TLeaf> PreparedTcpConnectOperation for TransportLeafTcpConnectOperation<TLeaf>
where
    TLeaf: ProtocolTransportLeaf
        + ProtocolTcpTransportLeafMetadata
        + ProtocolTcpTransportLeafOps
        + Send
        + Sync,
    TLeaf::Opened: ProtocolTcpTransportOpenResult,
{
    fn execute<'a>(
        self: Box<Self>,
        services: TcpRuntimeServices,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<EstablishedTcpOutbound, TcpOutboundFailure>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            let endpoint = self.prepared.endpoint();
            let tag = endpoint.tag.to_owned();
            let server = endpoint.server.to_owned();
            let port = endpoint.port;
            let dial_services = services.clone();
            let opened = open_prepared_tcp_transport_leaf_stream(
                session,
                &self.prepared,
                move |server, port| {
                    let services = dial_services.clone();
                    let server = server.to_owned();
                    async move { services.connect_upstream_owned(server, port).await }
                },
            )
            .await
            .map_err(|error| TcpOutboundFailure {
                stage: TLeaf::TCP_CONNECT_STAGE,
                error: error.into(),
                upstream_endpoint: Some((server.clone(), port)),
            })?;
            let (stream, traffic) = opened.into_proxied_stream_parts();
            if !traffic.is_empty() {
                services.record_control_traffic(session.id, traffic);
            }
            Ok(EstablishedTcpOutbound::proxied(tag, server, port, stream))
        })
    }
}

pub(crate) struct TransportLeafTcpRelayOperation<TLeaf> {
    pub(crate) prepared: PreparedTransportLeaf<TLeaf>,
}

impl<TLeaf> PreparedTcpRelayOperation for TransportLeafTcpRelayOperation<TLeaf>
where
    TLeaf: ProtocolTcpTransportLeafOps + Send + Sync,
{
    fn execute<'a>(
        self: Box<Self>,
        _services: TcpRuntimeServices,
        stream: TcpRelayStream,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<TcpRelayStream, EngineError>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            open_prepared_tcp_transport_leaf_relay_hop(stream, session, &self.prepared)
                .await
                .map_err(Into::into)
        })
    }
}
