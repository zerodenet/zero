use std::future::Future;
use std::pin::Pin;

use zero_core::Session;
use zero_engine::EngineError;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use zero_transport::outbound_leaf::{
    open_prepared_tcp_transport_bridge_relay_hop, open_prepared_tcp_transport_bridge_stream,
    PreparedTransportBridgeLeaf, ProtocolSessionTcpHandshake, ProtocolSocketTcpHandshake,
    ProtocolTcpTransportBridgeMetadata, ProtocolTcpTransportBridgeOps,
    ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
};

use crate::protocol_registry::TcpRuntimeServices;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

pub(crate) trait PreparedTcpConnectOperation: Send {
    fn execute<'a>(
        self: Box<Self>,
        services: TcpRuntimeServices,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<EstablishedTcpOutbound, TcpOutboundFailure>> + Send + 'a>>
    where
        Self: 'a;
}

pub(crate) trait PreparedTcpRelayOperation: Send {
    fn execute<'a>(
        self: Box<Self>,
        services: TcpRuntimeServices,
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
        services: TcpRuntimeServices,
        session: &'a Session,
    ) -> Pin<Box<dyn Future<Output = Result<EstablishedTcpOutbound, TcpOutboundFailure>> + Send + 'a>>
    where
        Self: 'a,
    {
        Box::pin(async move {
            execute_direct_tcp_operation(
                services,
                session,
                PreparedTcpOperation::Direct { tag: &self.tag },
            )
            .await
        })
    }
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct SocketTcpConnectOperation<T> {
    pub(crate) handshake: T,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
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

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct SocketTcpRelayOperation<T> {
    pub(crate) handshake: T,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
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

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct SessionTcpConnectOperation<T> {
    pub(crate) handshake: T,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
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

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct TransportBridgeTcpConnectOperation<TBridge, TLeaf> {
    pub(crate) bridge: TBridge,
    pub(crate) prepared: PreparedTransportBridgeLeaf<TLeaf>,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl<TBridge, TLeaf> PreparedTcpConnectOperation
    for TransportBridgeTcpConnectOperation<TBridge, TLeaf>
where
    TBridge:
        Send + Sync + ProtocolTcpTransportBridgeMetadata + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: ProtocolTransportLeaf + Send + Sync,
    TBridge::Opened: ProtocolTcpTransportOpenResult,
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
            let opened = open_prepared_tcp_transport_bridge_stream(
                &self.bridge,
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
                stage: TBridge::TCP_CONNECT_STAGE,
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

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct TransportBridgeTcpRelayOperation<TBridge, TLeaf> {
    pub(crate) bridge: TBridge,
    pub(crate) prepared: PreparedTransportBridgeLeaf<TLeaf>,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl<TBridge, TLeaf> PreparedTcpRelayOperation for TransportBridgeTcpRelayOperation<TBridge, TLeaf>
where
    TBridge: Send + Sync + ProtocolTcpTransportBridgeOps<TLeaf>,
    TLeaf: Send + Sync,
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
            open_prepared_tcp_transport_bridge_relay_hop(
                &self.bridge,
                stream,
                session,
                &self.prepared,
            )
            .await
            .map_err(Into::into)
        })
    }
}

pub(crate) enum PreparedTcpOperation<'a, 'leaf> {
    Direct {
        tag: &'a str,
    },
    #[doc(hidden)]
    _Lifetime(std::marker::PhantomData<&'leaf ()>),
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct PreparedSocketTcpOperation<'leaf, T> {
    pub(crate) handshake: &'leaf T,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) struct PreparedSessionTcpOperation<'leaf, T> {
    pub(crate) handshake: &'leaf T,
}

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
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

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) async fn execute_socket_tcp_connect_operation<T>(
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

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) async fn execute_socket_tcp_relay_hop_operation<T>(
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

pub(crate) async fn execute_direct_tcp_operation(
    services: TcpRuntimeServices,
    session: &Session,
    operation: PreparedTcpOperation<'_, '_>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
    let PreparedTcpOperation::Direct { tag } = operation else {
        unreachable!("direct TCP executor received an invalid operation")
    };
    match services.connect_direct(session).await {
        Ok(upstream) => Ok(EstablishedTcpOutbound::direct(tag, upstream.into())),
        Err(error) => Err(TcpOutboundFailure {
            stage: "connect_direct",
            error,
            upstream_endpoint: None,
        }),
    }
}
