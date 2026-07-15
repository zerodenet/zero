use std::future::Future;
use std::pin::Pin;

use zero_core::Session;
use zero_transport::outbound_leaf::ProtocolSessionTcpHandshake;

use super::contract::PreparedTcpConnectOperation;
use crate::protocol_registry::TcpRuntimeServices;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};

pub(crate) struct SessionTcpConnectOperation<T> {
    pub(crate) handshake: T,
}

impl<T> PreparedTcpConnectOperation for SessionTcpConnectOperation<T>
where
    T: ProtocolSessionTcpHandshake + Send + Sync,
{
    fn execute<'a>(
        self: Box<Self>,
        _services: TcpRuntimeServices,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<EstablishedTcpOutbound, TcpOutboundFailure>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            execute_session_tcp_connect_operation(
                session,
                PreparedSessionTcpOperation {
                    handshake: &self.handshake,
                },
            )
            .await
        })
    }
}

struct PreparedSessionTcpOperation<'leaf, T> {
    handshake: &'leaf T,
}

async fn execute_session_tcp_connect_operation<T>(
    session: &Session,
    operation: PreparedSessionTcpOperation<'_, T>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>
where
    T: ProtocolSessionTcpHandshake,
{
    let handshake = operation.handshake;
    let endpoint = (handshake.server().to_owned(), handshake.port());
    let stream = handshake
        .connect_session_stream(session)
        .await
        .map_err(|error| TcpOutboundFailure {
            stage: handshake.connect_stage(),
            error: error.into(),
            upstream_endpoint: Some(endpoint.clone()),
        })?;
    Ok(EstablishedTcpOutbound::proxied(
        handshake.tag().to_owned(),
        endpoint.0,
        endpoint.1,
        stream,
    ))
}
