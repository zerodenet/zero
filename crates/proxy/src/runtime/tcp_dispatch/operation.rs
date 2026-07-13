use std::future::Future;
use std::pin::Pin;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_transport::outbound_leaf::{
    open_prepared_tcp_transport_bridge_relay_hop, open_prepared_tcp_transport_bridge_stream,
    PreparedTransportBridgeLeaf, ProtocolSessionTcpHandshake, ProtocolSocketTcpHandshake,
    ProtocolTcpTransportBridgeMetadata, ProtocolTcpTransportBridgeOps,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
};

use crate::protocol_registry::{
    prepare_transport_bridge_leaf, OutboundAdapterContext, ProtocolTransportLeafResolver,
    ResolveTransportLeafError,
};
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

pub(crate) trait PreparedTcpConnectOperation: Send {
    fn execute<'a>(
        self: Box<Self>,
        ctx: OutboundAdapterContext<'a>,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<EstablishedTcpOutbound, TcpOutboundFailure>> + Send + 'a>>
    where
        Self: 'a;
}

pub(crate) trait PreparedTcpRelayOperation: Send {
    fn execute<'a>(
        self: Box<Self>,
        ctx: OutboundAdapterContext<'a>,
        stream: TcpRelayStream,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<TcpRelayStream, EngineError>> + Send + 'a>>
    where
        Self: 'a;
}

pub(crate) struct DirectTcpConnectOperation {
    pub(crate) tag: String,
}

impl PreparedTcpConnectOperation for DirectTcpConnectOperation {
    fn execute<'a>(
        self: Box<Self>,
        ctx: OutboundAdapterContext<'a>,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<EstablishedTcpOutbound, TcpOutboundFailure>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            execute_direct_tcp_operation(
                ctx,
                session,
                PreparedTcpOperation::Direct { tag: &self.tag },
            )
            .await
        })
    }
}

pub(crate) struct SocketTcpConnectOperation<T> {
    pub(crate) handshake: T,
}

impl<T> PreparedTcpConnectOperation for SocketTcpConnectOperation<T>
where
    T: ProtocolSocketTcpHandshake + Send + Sync,
{
    fn execute<'a>(
        self: Box<Self>,
        ctx: OutboundAdapterContext<'a>,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<EstablishedTcpOutbound, TcpOutboundFailure>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            execute_socket_tcp_connect_operation(
                ctx,
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
        _ctx: OutboundAdapterContext<'a>,
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

pub(crate) struct SessionTcpConnectOperation<T> {
    pub(crate) handshake: T,
}

impl<T> PreparedTcpConnectOperation for SessionTcpConnectOperation<T>
where
    T: ProtocolSessionTcpHandshake + Send + Sync,
{
    fn execute<'a>(
        self: Box<Self>,
        _ctx: OutboundAdapterContext<'a>,
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

pub(crate) struct TransportBridgeTcpConnectOperation<'a, TBridge, TLeaf> {
    bridge: &'a TBridge,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
}

impl<TBridge, TLeaf> PreparedTcpConnectOperation
    for TransportBridgeTcpConnectOperation<'_, TBridge, TLeaf>
where
    TBridge:
        Send + Sync + ProtocolTcpTransportBridgeMetadata + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send + Sync,
    TBridge::Opened: ProtocolTcpTransportOpenResult,
{
    fn execute<'a>(
        self: Box<Self>,
        ctx: OutboundAdapterContext<'a>,
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
            let proxy = ctx.proxy();
            let opened = open_prepared_tcp_transport_bridge_stream(
                self.bridge,
                session,
                &self.prepared,
                move |server, port| proxy.connect_upstream_host_owned(server.to_owned(), port),
            )
            .await
            .map_err(|error| TcpOutboundFailure {
                stage: TBridge::TCP_CONNECT_STAGE,
                error,
                upstream_endpoint: Some((server.clone(), port)),
            })?;
            let (stream, traffic) = opened.into_proxied_stream_parts();
            if !traffic.is_empty() {
                proxy.record_session_outbound_traffic(session.id, traffic);
            }
            Ok(EstablishedTcpOutbound::proxied(tag, server, port, stream))
        })
    }
}

pub(crate) struct TransportBridgeTcpRelayOperation<'a, TBridge, TLeaf> {
    bridge: &'a TBridge,
    prepared: PreparedTransportBridgeLeaf<TLeaf>,
}

impl<TBridge, TLeaf> PreparedTcpRelayOperation
    for TransportBridgeTcpRelayOperation<'_, TBridge, TLeaf>
where
    TBridge: Send + Sync + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: Send + Sync,
{
    fn execute<'a>(
        self: Box<Self>,
        _ctx: OutboundAdapterContext<'a>,
        stream: TcpRelayStream,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<TcpRelayStream, EngineError>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            open_prepared_tcp_transport_bridge_relay_hop(
                self.bridge,
                stream,
                session,
                &self.prepared,
            )
            .await
        })
    }
}

pub(crate) fn prepare_transport_bridge_tcp_connect<'a, TBridge>(
    bridge: &'a TBridge,
    source_dir: Option<&std::path::Path>,
    leaf: &'a ResolvedLeafOutbound<'a>,
) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure>
where
    TBridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf:
        ProtocolTransportLeaf + Send + Sync,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
    TBridge::Opened: ProtocolTcpTransportOpenResult,
{
    let prepared = prepare_transport_bridge_leaf(bridge, source_dir, leaf)
        .map_err(|error| connect_prepare_failure::<TBridge>(leaf, error))?;
    Ok(Box::new(TransportBridgeTcpConnectOperation {
        bridge,
        prepared,
    }))
}

pub(crate) fn prepare_transport_bridge_tcp_relay<'a, TBridge>(
    bridge: &'a TBridge,
    source_dir: Option<&std::path::Path>,
    leaf: &'a ResolvedLeafOutbound<'a>,
) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError>
where
    TBridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: Send + Sync,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    let prepared = prepare_transport_bridge_leaf(bridge, source_dir, leaf)
        .map_err(|error| relay_prepare_error::<TBridge, _>(error))?;
    Ok(Box::new(TransportBridgeTcpRelayOperation {
        bridge,
        prepared,
    }))
}

fn connect_prepare_failure<TBridge>(
    leaf: &ResolvedLeafOutbound<'_>,
    error: ResolveTransportLeafError<impl std::fmt::Display>,
) -> TcpOutboundFailure
where
    TBridge: ProtocolTcpTransportBridgeMetadata,
{
    let (stage, error, upstream_endpoint) = match error {
        ResolveTransportLeafError::InvalidConfig(error) => (
            TBridge::TCP_CONNECT_STAGE,
            invalid_input(TBridge::TCP_INVALID_CONNECT_CONFIG, error),
            leaf.proxy_endpoint()
                .map(|(server, port)| (server.to_owned(), port)),
        ),
        ResolveTransportLeafError::MissingLeaf => (
            TBridge::TCP_CONNECT_STAGE,
            invalid_input(
                TBridge::TCP_INVALID_CONNECT_LEAF_STAGE,
                TBridge::EXPECTED_OUTBOUND_LEAF,
            ),
            None,
        ),
    };
    TcpOutboundFailure {
        stage,
        error,
        upstream_endpoint,
    }
}

fn relay_prepare_error<TBridge, E>(error: ResolveTransportLeafError<E>) -> EngineError
where
    TBridge: ProtocolTcpTransportBridgeMetadata,
    E: std::fmt::Display,
{
    match error {
        ResolveTransportLeafError::InvalidConfig(error) => {
            invalid_input(TBridge::TCP_INVALID_RELAY_CONFIG, error)
        }
        ResolveTransportLeafError::MissingLeaf => invalid_input(
            TBridge::TCP_INVALID_RELAY_LEAF_STAGE,
            TBridge::EXPECTED_OUTBOUND_LEAF,
        ),
    }
}

fn invalid_input(stage: &'static str, error: impl std::fmt::Display) -> EngineError {
    EngineError::Io(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("{stage}: {error}"),
    ))
}

pub(crate) enum PreparedTcpOperation<'a, 'leaf> {
    Direct {
        tag: &'a str,
    },
    #[doc(hidden)]
    _Lifetime(std::marker::PhantomData<&'leaf ()>),
}

pub(crate) struct PreparedSocketTcpOperation<'leaf, T> {
    pub(crate) handshake: &'leaf T,
}

pub(crate) struct PreparedSessionTcpOperation<'leaf, T> {
    pub(crate) handshake: &'leaf T,
}

pub(crate) async fn execute_session_tcp_connect_operation<T>(
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
            error,
            upstream_endpoint: Some(endpoint.clone()),
        })?;
    Ok(EstablishedTcpOutbound::proxied(
        handshake.tag().to_owned(),
        endpoint.0,
        endpoint.1,
        stream,
    ))
}

pub(crate) async fn execute_socket_tcp_connect_operation<T>(
    ctx: OutboundAdapterContext<'_>,
    session: &Session,
    operation: PreparedSocketTcpOperation<'_, T>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>
where
    T: ProtocolSocketTcpHandshake,
{
    let proxy = ctx.proxy();
    let handshake = operation.handshake;
    let endpoint = (handshake.server().to_owned(), handshake.port());
    let socket = proxy
        .connect_upstream_host_owned(endpoint.0.clone(), endpoint.1)
        .await
        .map_err(|error| TcpOutboundFailure {
            stage: handshake.connect_stage(),
            error,
            upstream_endpoint: Some(endpoint.clone()),
        })?;
    let (stream, traffic) = handshake
        .handshake_socket(socket, session)
        .await
        .map_err(|error| TcpOutboundFailure {
            stage: handshake.connect_stage(),
            error,
            upstream_endpoint: Some(endpoint.clone()),
        })?;
    if !traffic.is_empty() {
        proxy.record_session_outbound_traffic(session.id, traffic);
    }
    Ok(EstablishedTcpOutbound::proxied(
        handshake.tag().to_owned(),
        endpoint.0,
        endpoint.1,
        stream,
    ))
}

pub(crate) async fn execute_socket_tcp_relay_hop_operation<T>(
    stream: TcpRelayStream,
    session: &Session,
    operation: PreparedSocketTcpOperation<'_, T>,
) -> Result<TcpRelayStream, EngineError>
where
    T: ProtocolSocketTcpHandshake,
{
    operation.handshake.handshake_relay(stream, session).await
}

pub(crate) async fn execute_direct_tcp_operation(
    ctx: OutboundAdapterContext<'_>,
    session: &Session,
    operation: PreparedTcpOperation<'_, '_>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
    let PreparedTcpOperation::Direct { tag } = operation else {
        unreachable!("direct TCP executor received an invalid operation")
    };
    let proxy = ctx.proxy();
    match proxy
        .protocols
        .direct_connector()
        .connect(session, proxy.resolver.as_ref())
        .await
    {
        Ok(upstream) => Ok(EstablishedTcpOutbound::direct(tag, upstream.into())),
        Err(error) => Err(TcpOutboundFailure {
            stage: "connect_direct",
            error: error.into(),
            upstream_endpoint: None,
        }),
    }
}
