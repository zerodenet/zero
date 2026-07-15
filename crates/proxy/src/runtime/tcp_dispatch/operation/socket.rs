use std::future::Future;
use std::pin::Pin;

use zero_core::Session;
use zero_engine::EngineError;
use zero_transport::outbound_leaf::ProtocolSocketTcpHandshake;

use super::contract::{PreparedTcpConnectOperation, PreparedTcpRelayOperation};
use crate::protocol_registry::TcpRuntimeServices;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

pub(crate) struct SocketTcpConnectOperation<T> {
    pub(crate) handshake: T,
}

impl<T> PreparedTcpConnectOperation for SocketTcpConnectOperation<T>
where
    T: ProtocolSocketTcpHandshake + Send + Sync,
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
            execute_socket_tcp_connect_operation(
                services,
                session,
                PreparedSocketTcpOperation {
                    handshake: &self.handshake,
                },
            )
            .await
        })
    }
}

pub(crate) struct SocketTcpRelayOperation<T> {
    pub(crate) handshake: T,
}

impl<T> PreparedTcpRelayOperation for SocketTcpRelayOperation<T>
where
    T: ProtocolSocketTcpHandshake + Send + Sync,
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
            execute_socket_tcp_relay_hop_operation(
                stream,
                session,
                PreparedSocketTcpOperation {
                    handshake: &self.handshake,
                },
            )
            .await
        })
    }
}

struct PreparedSocketTcpOperation<'leaf, T> {
    handshake: &'leaf T,
}

async fn execute_socket_tcp_connect_operation<T>(
    services: TcpRuntimeServices,
    session: &Session,
    operation: PreparedSocketTcpOperation<'_, T>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>
where
    T: ProtocolSocketTcpHandshake,
{
    let handshake = operation.handshake;
    let endpoint = (handshake.server().to_owned(), handshake.port());
    let socket = services
        .connect_upstream_owned(endpoint.0.clone(), endpoint.1)
        .await
        .map_err(|error| TcpOutboundFailure {
            stage: handshake.connect_stage(),
            error: error.into(),
            upstream_endpoint: Some(endpoint.clone()),
        })?;
    let (stream, traffic) = handshake
        .handshake_socket(socket, session)
        .await
        .map_err(|error| TcpOutboundFailure {
            stage: handshake.connect_stage(),
            error: error.into(),
            upstream_endpoint: Some(endpoint.clone()),
        })?;
    if !traffic.is_empty() {
        services.record_control_traffic(session.id, traffic);
    }
    Ok(EstablishedTcpOutbound::proxied(
        handshake.tag().to_owned(),
        endpoint.0,
        endpoint.1,
        stream,
    ))
}

async fn execute_socket_tcp_relay_hop_operation<T>(
    stream: TcpRelayStream,
    session: &Session,
    operation: PreparedSocketTcpOperation<'_, T>,
) -> Result<TcpRelayStream, EngineError>
where
    T: ProtocolSocketTcpHandshake,
{
    operation
        .handshake
        .handshake_relay(stream, session)
        .await
        .map_err(Into::into)
}
