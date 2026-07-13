use std::future::Future;
use std::pin::Pin;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_transport::outbound_leaf::{
    ProtocolSessionTcpHandshake, ProtocolSocketTcpHandshake, ProtocolTcpTransportBridgeMetadata,
    ProtocolTcpTransportBridgeOps, ProtocolTcpTransportOpenResult, ProtocolTransportLeaf,
    ProtocolTransportLeafResolver,
};

use crate::protocol_registry::OutboundAdapterContext;
use crate::transport::{
    apply_protocol_transport_bridge_relay_hop, connect_protocol_transport_bridge_tcp,
    EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream,
};

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

macro_rules! transport_bridge_tcp_operations {
    ($connect:ident, $relay:ident, $bridge:path) => {
        pub(crate) struct $connect<'a> {
            pub(crate) bridge: &'a $bridge,
            pub(crate) leaf: &'a ResolvedLeafOutbound<'a>,
        }

        impl<'a> PreparedTcpConnectOperation for $connect<'a> {
            fn execute<'b>(
                self: Box<Self>,
                ctx: OutboundAdapterContext<'b>,
                session: &'b Session,
            ) -> Pin<
                Box<
                    dyn Future<Output = Result<EstablishedTcpOutbound, TcpOutboundFailure>>
                        + Send
                        + 'b,
                >,
            >
            where
                Self: 'b,
            {
                Box::pin(async move {
                    execute_tcp_connect_operation(
                        self.bridge,
                        ctx,
                        session,
                        PreparedTcpOperation::Connect { leaf: self.leaf },
                    )
                    .await
                })
            }
        }

        pub(crate) struct $relay<'a> {
            pub(crate) bridge: &'a $bridge,
            pub(crate) leaf: &'a ResolvedLeafOutbound<'a>,
        }

        impl<'a> PreparedTcpRelayOperation for $relay<'a> {
            fn execute<'b>(
                self: Box<Self>,
                ctx: OutboundAdapterContext<'b>,
                stream: TcpRelayStream,
                session: &'b Session,
            ) -> Pin<Box<dyn Future<Output = Result<TcpRelayStream, EngineError>> + Send + 'b>>
            where
                Self: 'b,
            {
                Box::pin(async move {
                    execute_tcp_relay_hop_operation(
                        self.bridge,
                        ctx,
                        session,
                        PreparedTcpOperation::RelayHop {
                            stream,
                            leaf: self.leaf,
                        },
                    )
                    .await
                })
            }
        }
    };
}

#[cfg(feature = "vless")]
transport_bridge_tcp_operations!(
    VlessTcpConnectOperation,
    VlessTcpRelayOperation,
    zero_transport::vless_transport::VlessStreamBridge
);
#[cfg(feature = "vmess")]
transport_bridge_tcp_operations!(
    VmessTcpConnectOperation,
    VmessTcpRelayOperation,
    zero_transport::vmess_transport::VmessStreamBridge
);
#[cfg(feature = "trojan")]
transport_bridge_tcp_operations!(
    TrojanTcpConnectOperation,
    TrojanTcpRelayOperation,
    zero_transport::trojan_transport::TrojanTlsBridge
);

pub(crate) enum PreparedTcpOperation<'a, 'leaf> {
    Direct {
        tag: &'a str,
    },
    Connect {
        leaf: &'leaf ResolvedLeafOutbound<'a>,
    },
    RelayHop {
        stream: TcpRelayStream,
        leaf: &'leaf ResolvedLeafOutbound<'a>,
    },
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
        unreachable!("direct TCP executor received a protocol operation")
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

pub(crate) async fn execute_tcp_connect_operation<'a, TBridge>(
    bridge: &TBridge,
    ctx: OutboundAdapterContext<'_>,
    session: &Session,
    operation: PreparedTcpOperation<'a, '_>,
) -> Result<EstablishedTcpOutbound, TcpOutboundFailure>
where
    TBridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: ProtocolTransportLeaf,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
    <TBridge as ProtocolTcpTransportBridgeOps<
        <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf,
    >>::Opened: ProtocolTcpTransportOpenResult,
{
    let PreparedTcpOperation::Connect { leaf } = operation else {
        unreachable!("TCP connect executor received a relay-hop operation")
    };
    connect_protocol_transport_bridge_tcp(bridge, ctx, session, leaf, |_| {}).await
}

pub(crate) async fn execute_tcp_relay_hop_operation<'a, TBridge>(
    bridge: &TBridge,
    ctx: OutboundAdapterContext<'_>,
    session: &Session,
    operation: PreparedTcpOperation<'a, '_>,
) -> Result<TcpRelayStream, EngineError>
where
    TBridge: Send
        + Sync
        + ProtocolTransportLeafResolver<'a>
        + ProtocolTcpTransportBridgeMetadata
        + ProtocolTcpTransportBridgeOps<<TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf>,
    <TBridge as ProtocolTransportLeafResolver<'a>>::TransportLeaf: Sync,
    <TBridge as ProtocolTransportLeafResolver<'a>>::ResolveError: std::fmt::Display,
{
    let PreparedTcpOperation::RelayHop { stream, leaf } = operation else {
        unreachable!("TCP relay executor received a connect operation")
    };
    apply_protocol_transport_bridge_relay_hop(bridge, ctx, stream, session, leaf).await
}
