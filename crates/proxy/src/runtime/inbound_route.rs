use core::future::Future;
use std::path::Path;

use tokio::io::{AsyncRead, AsyncWrite};
use tracing::warn;
use zero_config::{InboundConfig, InboundProtocolConfig};
use zero_core::{
    InboundClientResponse, InboundMuxServer, InboundMuxStreamRoute, InboundStreamRoute, Session,
    StreamUdpResponder,
};
use zero_engine::EngineError;
#[cfg(feature = "transport_quic")]
use zero_transport::inbound_route::RecordedBoundMuxRouteRequest;
use zero_transport::inbound_route::{
    FallbackReplayToUpstream, MuxRouteRequest, ProtocolInboundRequestFactory,
    ProtocolMuxRouteDispatchMetadata, ProtocolStreamRouteDispatchMetadata,
    RecordedProtocolMuxRouteDispatchMetadata, RouteAcceptResult, StreamRouteRequest,
};

use crate::protocol_registry::BoundInbound;
use crate::runtime::inbound_protocol::{
    serve_inbound, ClientResponseInboundProtocol, InboundProtocol, NoClientResponseInboundProtocol,
};
#[cfg(feature = "transport_quic")]
use crate::runtime::listener_loop::spawn_logged_bound_inbound_listener;
use crate::runtime::listener_loop::spawn_logged_tcp_inbound_listener;
use crate::runtime::mux_session::{run_protocol_mux_session, MuxSessionLoop};
use crate::runtime::mux_tcp::run_protocol_mux_tcp_task;
use crate::runtime::mux_udp::{run_logged_protocol_mux_udp_relay, run_protocol_mux_udp_task};
use crate::runtime::stream_udp::run_mapped_protocol_stream_udp_relay;
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream, RecordingStream, TcpRelayStream};

pub(crate) struct StreamRouteBridge<P, FMapTcp, FRunUdp> {
    pub(crate) proxy: Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) source_addr: Option<std::net::SocketAddr>,
    pub(crate) protocol: P,
    pub(crate) map_tcp_stream: FMapTcp,
    pub(crate) run_udp: FRunUdp,
}

pub(crate) async fn dispatch_protocol_stream_route<R, P, FMapTcp, FRunUdp, FUdp>(
    route: R,
    request: StreamRouteBridge<P, FMapTcp, FRunUdp>,
) -> Result<(), EngineError>
where
    R: InboundStreamRoute,
    R::TcpStream: Send,
    P: InboundProtocol + 'static,
    FMapTcp: FnOnce(R::TcpStream) -> P::ClientStream + Send,
    FRunUdp: FnOnce(Proxy, Session, R::UdpRelay, String) -> FUdp + Send,
    FUdp: Future<Output = Result<(), EngineError>> + Send,
{
    let StreamRouteBridge {
        proxy,
        inbound_tag,
        source_addr,
        protocol,
        map_tcp_stream,
        run_udp,
    } = request;

    let tcp_proxy = proxy.clone();
    let udp_proxy = proxy.clone();
    let tcp_inbound_tag = inbound_tag.clone();

    route
        .dispatch_inbound_route(
            move |session, stream| async move {
                serve_inbound(
                    &tcp_proxy,
                    session,
                    map_tcp_stream(stream),
                    &protocol,
                    &tcp_inbound_tag,
                    source_addr,
                )
                .await
            },
            move |session, relay| run_udp(udp_proxy, session, relay, inbound_tag),
        )
        .await
}

pub(crate) async fn dispatch_no_client_stream_route<R>(
    route: R,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    udp_protocol: &'static str,
) -> Result<(), EngineError>
where
    R: InboundStreamRoute,
    R::TcpStream: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    R::UdpRelay: zero_core::InboundStreamUdpRelay,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
{
    dispatch_protocol_stream_route(
        route,
        StreamRouteBridge {
            proxy,
            inbound_tag,
            source_addr,
            protocol: NoClientResponseInboundProtocol,
            map_tcp_stream: TcpRelayStream::new,
            run_udp: move |proxy: Proxy,
                           session: Session,
                           relay: R::UdpRelay,
                           inbound_tag: String| async move {
                run_mapped_protocol_stream_udp_relay(
                    &proxy,
                    &session,
                    relay,
                    &inbound_tag,
                    udp_protocol,
                    TcpRelayStream::new,
                    None,
                )
                .await
            },
        },
    )
    .await
}

pub(crate) async fn dispatch_no_client_stream_route_request<Q>(
    proxy: Proxy,
    request: Q,
    inbound_tag: String,
    socket: zero_platform_tokio::TokioSocket,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError>
where
    Q: StreamRouteRequest + ProtocolStreamRouteDispatchMetadata,
    Q::Route: InboundStreamRoute,
    <Q::Route as InboundStreamRoute>::TcpStream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <Q::Route as InboundStreamRoute>::UdpRelay: zero_core::InboundStreamUdpRelay,
    <<Q::Route as InboundStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <<Q::Route as InboundStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
{
    let route = request.accept_route(socket).await?;
    dispatch_no_client_stream_route(route, proxy, inbound_tag, source_addr, Q::UDP_PROTOCOL).await
}

pub(crate) fn spawn_no_client_stream_route_inbound_listener<Q, B>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    build_request: B,
) where
    Q: Clone + Send + Sync + 'static + StreamRouteRequest + ProtocolStreamRouteDispatchMetadata,
    Q::Route: InboundStreamRoute,
    <Q::Route as InboundStreamRoute>::TcpStream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <Q::Route as InboundStreamRoute>::UdpRelay: zero_core::InboundStreamUdpRelay,
    <<Q::Route as InboundStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <<Q::Route as InboundStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    B: FnOnce(&InboundProtocolConfig, Option<&Path>) -> Result<Q, EngineError> + Send + 'static,
{
    spawn_logged_tcp_inbound_listener(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        build_request,
        |request: &Q| request.protocol_name(),
        Q::ERROR_PROTOCOL_NAME,
        |proxy: Proxy,
         request: Q,
         inbound_tag: String,
         socket: zero_platform_tokio::TokioSocket,
         source_addr: Option<std::net::SocketAddr>| {
            dispatch_no_client_stream_route_request(
                proxy,
                request,
                inbound_tag,
                socket,
                source_addr,
            )
        },
    );
}

pub(crate) fn spawn_transport_stream_route_inbound_listener<Q, B>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    build_request: B,
) where
    Q: Clone + Send + Sync + 'static + StreamRouteRequest + ProtocolStreamRouteDispatchMetadata,
    Q::Route: InboundStreamRoute,
    <Q::Route as InboundStreamRoute>::TcpStream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <Q::Route as InboundStreamRoute>::UdpRelay: zero_core::InboundStreamUdpRelay,
    <<Q::Route as InboundStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <<Q::Route as InboundStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    B: FnOnce(&InboundProtocolConfig, Option<&Path>) -> Result<Q, EngineError> + Send + 'static,
{
    spawn_no_client_stream_route_inbound_listener(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        build_request,
    );
}

pub(crate) fn spawn_transport_stream_route_inbound_listener_with_request<Q>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
) where
    Q: Clone
        + Send
        + Sync
        + 'static
        + StreamRouteRequest
        + ProtocolStreamRouteDispatchMetadata
        + ProtocolInboundRequestFactory,
    Q::Route: InboundStreamRoute,
    <Q::Route as InboundStreamRoute>::TcpStream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <Q::Route as InboundStreamRoute>::UdpRelay: zero_core::InboundStreamUdpRelay,
    <<Q::Route as InboundStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <<Q::Route as InboundStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
{
    spawn_transport_stream_route_inbound_listener::<Q, _>(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        |protocol, source_dir| {
            <Q as ProtocolInboundRequestFactory>::from_protocol_config(protocol, source_dir)
        },
    );
}

pub(crate) struct MuxRouteBridge<P, FMapTcp, FRunUdp, FRunMux> {
    pub(crate) proxy: Proxy,
    pub(crate) inbound_tag: String,
    pub(crate) source_addr: Option<std::net::SocketAddr>,
    pub(crate) protocol: P,
    pub(crate) map_tcp_stream: FMapTcp,
    pub(crate) run_udp: FRunUdp,
    pub(crate) run_mux: FRunMux,
}

#[derive(Clone, Copy)]
pub(crate) struct NoClientMuxRouteDefaults {
    pub(crate) udp_protocol: &'static str,
    pub(crate) mux_protocol: &'static str,
    pub(crate) panic_message: &'static str,
    pub(crate) abort_on_end: bool,
    pub(crate) read_error_log: &'static str,
}

pub(crate) async fn dispatch_protocol_mux_route<R, P, FMapTcp, FRunUdp, FUdp, FRunMux, FMux>(
    route: R,
    request: MuxRouteBridge<P, FMapTcp, FRunUdp, FRunMux>,
) -> Result<(), EngineError>
where
    R: InboundMuxStreamRoute,
    R::TcpStream: Send,
    R::MuxReader: Send,
    P: InboundProtocol + 'static,
    FMapTcp: FnOnce(R::TcpStream) -> P::ClientStream + Send,
    FRunUdp: FnOnce(Proxy, Session, R::UdpRelay, String) -> FUdp + Send,
    FUdp: Future<Output = Result<(), EngineError>> + Send,
    FRunMux: FnOnce(Proxy, R::MuxReader, R::MuxServer, String) -> FMux + Send,
    FMux: Future<Output = Result<(), EngineError>> + Send,
{
    let MuxRouteBridge {
        proxy,
        inbound_tag,
        source_addr,
        protocol,
        map_tcp_stream,
        run_udp,
        run_mux,
    } = request;

    let tcp_proxy = proxy.clone();
    let udp_proxy = proxy.clone();
    let mux_proxy = proxy.clone();
    let tcp_inbound_tag = inbound_tag.clone();
    let udp_inbound_tag = inbound_tag.clone();

    route
        .dispatch_inbound_route(
            move |session, stream| async move {
                serve_inbound(
                    &tcp_proxy,
                    session,
                    map_tcp_stream(stream),
                    &protocol,
                    &tcp_inbound_tag,
                    source_addr,
                )
                .await
            },
            move |session, relay| run_udp(udp_proxy, session, relay, udp_inbound_tag),
            move |reader, mux_server| run_mux(mux_proxy, reader, mux_server, inbound_tag),
        )
        .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn dispatch_no_client_mux_route<R, FTcp, FTcpFut, FUdp, FUdpFut>(
    route: R,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    udp_protocol: &'static str,
    mux_protocol: &'static str,
    panic_message: &'static str,
    abort_on_end: bool,
    read_error_log: &'static str,
    mut spawn_tcp: FTcp,
    mut spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    R: InboundMuxStreamRoute,
    R::TcpStream: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    R::UdpRelay: zero_core::InboundStreamUdpRelay,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    R::MuxServer: InboundMuxServer<R::MuxReader>,
    R::MuxReader: Send,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<R::MuxReader>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(Proxy, <R::MuxServer as InboundMuxServer<R::MuxReader>>::UdpRelay, String) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    dispatch_protocol_mux_route(
        route,
        MuxRouteBridge {
            proxy,
            inbound_tag,
            source_addr,
            protocol: NoClientResponseInboundProtocol,
            map_tcp_stream: TcpRelayStream::new,
            run_udp: move |proxy: Proxy,
                           session: Session,
                           relay: R::UdpRelay,
                           inbound_tag: String| async move {
                run_mapped_protocol_stream_udp_relay(
                    &proxy,
                    &session,
                    relay,
                    &inbound_tag,
                    udp_protocol,
                    TcpRelayStream::new,
                    None,
                )
                .await
            },
            run_mux: move |proxy: Proxy,
                           reader: R::MuxReader,
                           mux_server: R::MuxServer,
                           inbound_tag: String| async move {
                match run_protocol_mux_session(
                    &proxy,
                    reader,
                    mux_server,
                    MuxSessionLoop {
                        inbound_tag: &inbound_tag,
                        protocol: mux_protocol,
                        panic_message,
                        abort_on_end,
                    },
                    |proxy, session, relay, inbound_tag| {
                        spawn_tcp(proxy, session, relay, inbound_tag)
                    },
                    |proxy, relay, inbound_tag| spawn_udp(proxy, relay, inbound_tag),
                )
                .await
                {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        warn!(error = %error, "{read_error_log}");
                        Ok(())
                    }
                }
            },
        },
    )
    .await
}

pub(crate) async fn dispatch_no_client_mux_route_with_defaults<R, FTcp, FTcpFut, FUdp, FUdpFut>(
    route: R,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    defaults: NoClientMuxRouteDefaults,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    R: InboundMuxStreamRoute,
    R::TcpStream: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    R::UdpRelay: zero_core::InboundStreamUdpRelay,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    R::MuxServer: InboundMuxServer<R::MuxReader>,
    R::MuxReader: Send,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<R::MuxReader>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(Proxy, <R::MuxServer as InboundMuxServer<R::MuxReader>>::UdpRelay, String) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    dispatch_no_client_mux_route(
        route,
        proxy,
        inbound_tag,
        source_addr,
        defaults.udp_protocol,
        defaults.mux_protocol,
        defaults.panic_message,
        defaults.abort_on_end,
        defaults.read_error_log,
        spawn_tcp,
        spawn_udp,
    )
    .await
}

pub(crate) async fn dispatch_no_client_mux_route_request_with_defaults<
    Q,
    FTcp,
    FTcpFut,
    FUdp,
    FUdpFut,
>(
    proxy: Proxy,
    request: Q,
    inbound_tag: String,
    socket: zero_platform_tokio::TokioSocket,
    source_addr: Option<std::net::SocketAddr>,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    Q: MuxRouteRequest + ProtocolMuxRouteDispatchMetadata,
    Q::Route: InboundMuxStreamRoute,
    <Q::Route as InboundMuxStreamRoute>::TcpStream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <Q::Route as InboundMuxStreamRoute>::UdpRelay: zero_core::InboundStreamUdpRelay,
    <<Q::Route as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <<Q::Route as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    <Q::Route as InboundMuxStreamRoute>::MuxServer:
        InboundMuxServer<<Q::Route as InboundMuxStreamRoute>::MuxReader>,
    <Q::Route as InboundMuxStreamRoute>::MuxReader: Send,
    FTcp: FnMut(
            Proxy,
            Session,
            <<Q::Route as InboundMuxStreamRoute>::MuxServer as InboundMuxServer<
                <Q::Route as InboundMuxStreamRoute>::MuxReader,
            >>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            Proxy,
            <<Q::Route as InboundMuxStreamRoute>::MuxServer as InboundMuxServer<
                <Q::Route as InboundMuxStreamRoute>::MuxReader,
            >>::UdpRelay,
            String,
        ) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    let route = request.accept_route(socket).await?;
    dispatch_no_client_mux_route_with_defaults(
        route,
        proxy,
        inbound_tag,
        source_addr,
        NoClientMuxRouteDefaults {
            udp_protocol: Q::UDP_PROTOCOL,
            mux_protocol: Q::MUX_PROTOCOL,
            panic_message: Q::PANIC_MESSAGE,
            abort_on_end: Q::ABORT_ON_END,
            read_error_log: Q::READ_ERROR_LOG,
        },
        spawn_tcp,
        spawn_udp,
    )
    .await
}

pub(crate) fn spawn_no_client_mux_route_inbound_listener<Q, B, FTcp, FTcpFut, FUdp, FUdpFut>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    build_request: B,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) where
    Q: Clone + Send + Sync + 'static + MuxRouteRequest + ProtocolMuxRouteDispatchMetadata,
    Q::Route: InboundMuxStreamRoute,
    <Q::Route as InboundMuxStreamRoute>::TcpStream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <Q::Route as InboundMuxStreamRoute>::UdpRelay: zero_core::InboundStreamUdpRelay,
    <<Q::Route as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <<Q::Route as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    <Q::Route as InboundMuxStreamRoute>::MuxServer:
        InboundMuxServer<<Q::Route as InboundMuxStreamRoute>::MuxReader>,
    <Q::Route as InboundMuxStreamRoute>::MuxReader: Send,
    B: FnOnce(&InboundProtocolConfig, Option<&Path>) -> Result<Q, EngineError> + Send + 'static,
    FTcp: Fn(
            Proxy,
            Session,
            <<Q::Route as InboundMuxStreamRoute>::MuxServer as InboundMuxServer<
                <Q::Route as InboundMuxStreamRoute>::MuxReader,
            >>::TcpRelay,
            String,
        ) -> FTcpFut
        + Clone
        + Send
        + Sync
        + 'static,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: Fn(
            Proxy,
            <<Q::Route as InboundMuxStreamRoute>::MuxServer as InboundMuxServer<
                <Q::Route as InboundMuxStreamRoute>::MuxReader,
            >>::UdpRelay,
            String,
        ) -> FUdpFut
        + Clone
        + Send
        + Sync
        + 'static,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    spawn_logged_tcp_inbound_listener(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        build_request,
        |request: &Q| request.protocol_name(),
        Q::ERROR_PROTOCOL_NAME,
        move |proxy: Proxy,
              request: Q,
              inbound_tag: String,
              socket: zero_platform_tokio::TokioSocket,
              source_addr: Option<std::net::SocketAddr>| {
            dispatch_no_client_mux_route_request_with_defaults(
                proxy,
                request,
                inbound_tag,
                socket,
                source_addr,
                spawn_tcp.clone(),
                spawn_udp.clone(),
            )
        },
    );
}

pub(crate) fn spawn_transport_mux_route_inbound_listener<Q, B>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    build_request: B,
) where
    Q: Clone + Send + Sync + 'static + MuxRouteRequest + ProtocolMuxRouteDispatchMetadata,
    Q::Route: InboundMuxStreamRoute,
    <Q::Route as InboundMuxStreamRoute>::TcpStream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <Q::Route as InboundMuxStreamRoute>::UdpRelay: zero_core::InboundStreamUdpRelay,
    <<Q::Route as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <<Q::Route as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    <Q::Route as InboundMuxStreamRoute>::MuxServer:
        InboundMuxServer<<Q::Route as InboundMuxStreamRoute>::MuxReader>,
    <Q::Route as InboundMuxStreamRoute>::MuxReader: Send,
    B: FnOnce(&InboundProtocolConfig, Option<&Path>) -> Result<Q, EngineError> + Send + 'static,
{
    spawn_no_client_mux_route_inbound_listener(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        build_request,
        move |proxy, session, relay, inbound_tag| {
            run_protocol_mux_tcp_task(proxy, session, relay, inbound_tag, Q::MUX_PROTOCOL)
        },
        move |proxy, relay, inbound_tag| {
            run_protocol_mux_udp_task(proxy, relay, inbound_tag, Q::UDP_PROTOCOL)
        },
    );
}

pub(crate) fn spawn_transport_mux_route_inbound_listener_with_request<Q>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
) where
    Q: Clone
        + Send
        + Sync
        + 'static
        + MuxRouteRequest
        + ProtocolMuxRouteDispatchMetadata
        + ProtocolInboundRequestFactory,
    Q::Route: InboundMuxStreamRoute,
    <Q::Route as InboundMuxStreamRoute>::TcpStream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <Q::Route as InboundMuxStreamRoute>::UdpRelay: zero_core::InboundStreamUdpRelay,
    <<Q::Route as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <<Q::Route as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    <Q::Route as InboundMuxStreamRoute>::MuxServer:
        InboundMuxServer<<Q::Route as InboundMuxStreamRoute>::MuxReader>,
    <Q::Route as InboundMuxStreamRoute>::MuxReader: Send,
{
    spawn_transport_mux_route_inbound_listener::<Q, _>(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        |protocol, source_dir| {
            <Q as ProtocolInboundRequestFactory>::from_protocol_config(protocol, source_dir)
        },
    );
}

#[cfg(feature = "transport_quic")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_recorded_protocol_mux_bound_inbound_listener<
    Q,
    B,
    STcp,
    RTcp,
    FRTcp,
    SQuic,
    RQuic,
    FRQuic,
    FTcpAccept,
    FTcpAcceptFut,
    FQuicAccept,
    FQuicAcceptFut,
    FTcp,
    FTcpFut,
    FUdp,
    FUdpFut,
>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    build_request: B,
    accept_tcp_route: FTcpAccept,
    accept_quic_route: FQuicAccept,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) where
    Q: Clone + Send + Sync + 'static + RecordedProtocolMuxRouteDispatchMetadata,
    B: FnOnce(&InboundProtocolConfig, Option<&Path>) -> Result<Q, EngineError> + Send + 'static,
    Q::ResponseProtocol: InboundClientResponse<TcpRelayStream> + Clone + Send + Sync + 'static,
    STcp: ClientStream + 'static,
    RTcp: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<STcp>>,
            MuxReader = MeteredStream<RecordingStream<STcp>>,
        > + Send
        + 'static,
    RTcp::UdpRelay: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<STcp>>>,
    RTcp::MuxServer: InboundMuxServer<MeteredStream<STcp>>,
    <RTcp::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<STcp>>,
    RTcp::MuxReader: Send,
    FRTcp: FallbackReplayToUpstream + 'static,
    SQuic: ClientStream + 'static,
    RQuic: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<SQuic>>,
            MuxReader = MeteredStream<RecordingStream<SQuic>>,
        > + Send
        + 'static,
    RQuic::UdpRelay:
        zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<SQuic>>>,
    RQuic::MuxServer: InboundMuxServer<MeteredStream<SQuic>>,
    <RQuic::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<SQuic>>,
    RQuic::MuxReader: Send,
    FRQuic: FallbackReplayToUpstream + 'static,
    FTcpAccept:
        Fn(Q, zero_platform_tokio::TokioSocket) -> FTcpAcceptFut + Clone + Send + Sync + 'static,
    FTcpAcceptFut:
        Future<Output = Result<Option<RouteAcceptResult<RTcp, FRTcp>>, EngineError>> + Send,
    FQuicAccept:
        Fn(Q, crate::transport::QuicStream) -> FQuicAcceptFut + Clone + Send + Sync + 'static,
    FQuicAcceptFut: Future<Output = Result<RouteAcceptResult<RQuic, FRQuic>, EngineError>> + Send,
    FTcp: Fn(
            Proxy,
            Session,
            <RTcp::MuxServer as InboundMuxServer<MeteredStream<STcp>>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Clone
        + Send
        + Sync
        + 'static,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: Fn(
            Proxy,
            <RTcp::MuxServer as InboundMuxServer<MeteredStream<STcp>>>::UdpRelay,
            String,
        ) -> FUdpFut
        + Clone
        + Send
        + Sync
        + 'static,
    FUdpFut: Future<Output = ()> + Send + 'static,
    <RTcp::MuxServer as InboundMuxServer<MeteredStream<STcp>>>::TcpRelay:
        From<<RQuic::MuxServer as InboundMuxServer<MeteredStream<SQuic>>>::TcpRelay>,
    <RTcp::MuxServer as InboundMuxServer<MeteredStream<STcp>>>::UdpRelay:
        From<<RQuic::MuxServer as InboundMuxServer<MeteredStream<SQuic>>>::UdpRelay>,
{
    let tcp_accept_route = accept_tcp_route.clone();
    let quic_accept_route = accept_quic_route.clone();
    let tcp_spawn_tcp = spawn_tcp.clone();
    let quic_spawn_tcp = spawn_tcp;
    let tcp_spawn_udp = spawn_udp.clone();
    let quic_spawn_udp = spawn_udp;

    spawn_logged_bound_inbound_listener(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        build_request,
        |request: &Q| request.protocol_name(),
        Q::ERROR_PROTOCOL_NAME,
        move |proxy: Proxy,
              request: Q,
              inbound_tag: String,
              socket: zero_platform_tokio::TokioSocket,
              source_addr: Option<std::net::SocketAddr>| {
            let protocol = ClientResponseInboundProtocol::new(request.response_protocol());
            let accept_tcp_route = tcp_accept_route.clone();
            let spawn_tcp = tcp_spawn_tcp.clone();
            let spawn_udp = tcp_spawn_udp.clone();
            async move {
                dispatch_recorded_protocol_mux_tcp_request_result(
                    accept_tcp_route(request, socket).await,
                    proxy,
                    inbound_tag,
                    source_addr,
                    protocol,
                    recorded_protocol_mux_defaults::<Q>(),
                    spawn_tcp,
                    spawn_udp,
                )
                .await
            }
        },
        move |proxy: Proxy,
              request: Q,
              inbound_tag: String,
              stream: crate::transport::QuicStream| {
            let protocol = ClientResponseInboundProtocol::new(request.response_protocol());
            let accept_quic_route = quic_accept_route.clone();
            let spawn_tcp = quic_spawn_tcp.clone();
            let spawn_udp = quic_spawn_udp.clone();
            async move {
                dispatch_recorded_protocol_mux_stream_request_result(
                    accept_quic_route(request, stream).await,
                    proxy,
                    inbound_tag,
                    None,
                    protocol,
                    recorded_protocol_mux_defaults::<Q>(),
                    move |proxy, session, relay, inbound_tag| {
                        spawn_tcp(proxy, session, relay.into(), inbound_tag)
                    },
                    move |proxy, relay, inbound_tag| spawn_udp(proxy, relay.into(), inbound_tag),
                )
                .await
            }
        },
    );
}

#[cfg(feature = "transport_quic")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_recorded_transport_mux_bound_inbound_listener<
    Q,
    B,
    STcp,
    RTcp,
    FRTcp,
    SQuic,
    RQuic,
    FRQuic,
    FTcpAccept,
    FTcpAcceptFut,
    FQuicAccept,
    FQuicAcceptFut,
>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    build_request: B,
    accept_tcp_route: FTcpAccept,
    accept_quic_route: FQuicAccept,
) where
    Q: Clone + Send + Sync + 'static + RecordedProtocolMuxRouteDispatchMetadata,
    Q::ResponseProtocol: InboundClientResponse<TcpRelayStream> + Clone + Send + Sync + 'static,
    STcp: ClientStream + 'static,
    RTcp: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<STcp>>,
            MuxReader = MeteredStream<RecordingStream<STcp>>,
        > + Send
        + 'static,
    RTcp::UdpRelay: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<STcp>>>,
    RTcp::MuxServer: InboundMuxServer<MeteredStream<STcp>>,
    <RTcp::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<STcp>>,
    RTcp::MuxReader: Send,
    FRTcp: FallbackReplayToUpstream + 'static,
    SQuic: ClientStream + 'static,
    RQuic: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<SQuic>>,
            MuxReader = MeteredStream<RecordingStream<SQuic>>,
        > + Send
        + 'static,
    RQuic::UdpRelay:
        zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<SQuic>>>,
    RQuic::MuxServer: InboundMuxServer<MeteredStream<SQuic>>,
    <RQuic::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<SQuic>>,
    RQuic::MuxReader: Send,
    FRQuic: FallbackReplayToUpstream + 'static,
    FTcpAccept:
        Fn(Q, zero_platform_tokio::TokioSocket) -> FTcpAcceptFut + Clone + Send + Sync + 'static,
    FTcpAcceptFut:
        Future<Output = Result<Option<RouteAcceptResult<RTcp, FRTcp>>, EngineError>> + Send,
    FQuicAccept:
        Fn(Q, crate::transport::QuicStream) -> FQuicAcceptFut + Clone + Send + Sync + 'static,
    FQuicAcceptFut: Future<Output = Result<RouteAcceptResult<RQuic, FRQuic>, EngineError>> + Send,
    <RTcp::MuxServer as InboundMuxServer<MeteredStream<STcp>>>::TcpRelay:
        From<<RQuic::MuxServer as InboundMuxServer<MeteredStream<SQuic>>>::TcpRelay>,
    <RTcp::MuxServer as InboundMuxServer<MeteredStream<STcp>>>::UdpRelay:
        From<<RQuic::MuxServer as InboundMuxServer<MeteredStream<SQuic>>>::UdpRelay>,
    B: FnOnce(&InboundProtocolConfig, Option<&Path>) -> Result<Q, EngineError> + Send + 'static,
{
    spawn_recorded_protocol_mux_bound_inbound_listener(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        build_request,
        accept_tcp_route,
        accept_quic_route,
        move |proxy, session, relay, inbound_tag| {
            run_protocol_mux_tcp_task(proxy, session, relay, inbound_tag, Q::MUX_PROTOCOL)
        },
        move |proxy, relay, inbound_tag| async move {
            run_logged_protocol_mux_udp_relay(
                proxy,
                relay,
                inbound_tag,
                Q::UDP_PROTOCOL,
                |inbound_tag, _relay| {
                    tracing::info!(
                        inbound_tag = %inbound_tag,
                        network = "udp",
                        "MUX stream accepted"
                    );
                },
            )
            .await
        },
    );
}

#[cfg(feature = "transport_quic")]
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_recorded_transport_mux_bound_inbound_listener_with_request<
    Q,
    STcp,
    RTcp,
    FRTcp,
    SQuic,
    RQuic,
    FRQuic,
    FTcpAccept,
    FTcpAcceptFut,
    FQuicAccept,
    FQuicAcceptFut,
>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    accept_tcp_route: FTcpAccept,
    accept_quic_route: FQuicAccept,
) where
    Q: Clone
        + Send
        + Sync
        + 'static
        + RecordedProtocolMuxRouteDispatchMetadata
        + ProtocolInboundRequestFactory,
    Q::ResponseProtocol: InboundClientResponse<TcpRelayStream> + Clone + Send + Sync + 'static,
    STcp: ClientStream + 'static,
    RTcp: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<STcp>>,
            MuxReader = MeteredStream<RecordingStream<STcp>>,
        > + Send
        + 'static,
    RTcp::UdpRelay: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<STcp>>>,
    RTcp::MuxServer: InboundMuxServer<MeteredStream<STcp>>,
    <RTcp::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<STcp>>,
    RTcp::MuxReader: Send,
    FRTcp: FallbackReplayToUpstream + 'static,
    SQuic: ClientStream + 'static,
    RQuic: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<SQuic>>,
            MuxReader = MeteredStream<RecordingStream<SQuic>>,
        > + Send
        + 'static,
    RQuic::UdpRelay:
        zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<SQuic>>>,
    RQuic::MuxServer: InboundMuxServer<MeteredStream<SQuic>>,
    <RQuic::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<SQuic>>,
    RQuic::MuxReader: Send,
    FRQuic: FallbackReplayToUpstream + 'static,
    FTcpAccept:
        Fn(Q, zero_platform_tokio::TokioSocket) -> FTcpAcceptFut + Clone + Send + Sync + 'static,
    FTcpAcceptFut:
        Future<Output = Result<Option<RouteAcceptResult<RTcp, FRTcp>>, EngineError>> + Send,
    FQuicAccept:
        Fn(Q, crate::transport::QuicStream) -> FQuicAcceptFut + Clone + Send + Sync + 'static,
    FQuicAcceptFut: Future<Output = Result<RouteAcceptResult<RQuic, FRQuic>, EngineError>> + Send,
    <RTcp::MuxServer as InboundMuxServer<MeteredStream<STcp>>>::TcpRelay:
        From<<RQuic::MuxServer as InboundMuxServer<MeteredStream<SQuic>>>::TcpRelay>,
    <RTcp::MuxServer as InboundMuxServer<MeteredStream<STcp>>>::UdpRelay:
        From<<RQuic::MuxServer as InboundMuxServer<MeteredStream<SQuic>>>::UdpRelay>,
{
    spawn_recorded_transport_mux_bound_inbound_listener::<
        Q,
        _,
        STcp,
        RTcp,
        FRTcp,
        SQuic,
        RQuic,
        FRQuic,
        FTcpAccept,
        FTcpAcceptFut,
        FQuicAccept,
        FQuicAcceptFut,
    >(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        |protocol, source_dir| {
            <Q as ProtocolInboundRequestFactory>::from_protocol_config(protocol, source_dir)
        },
        accept_tcp_route,
        accept_quic_route,
    );
}

#[cfg(feature = "transport_quic")]
pub(crate) fn spawn_recorded_transport_mux_bound_inbound_listener_for_request<Q>(
    proxy: &Proxy,
    inbound: InboundConfig,
    bound: BoundInbound,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
    listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
) where
    Q: RecordedBoundMuxRouteRequest,
    Q::ResponseProtocol: InboundClientResponse<TcpRelayStream> + Clone + Send + Sync + 'static,
    Q::TcpRoute: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<Q::TcpStream>>,
            MuxReader = MeteredStream<RecordingStream<Q::TcpStream>>,
        > + Send
        + 'static,
    <Q::TcpRoute as InboundMuxStreamRoute>::UdpRelay:
        zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<Q::TcpStream>>>,
    <Q::TcpRoute as InboundMuxStreamRoute>::MuxServer:
        InboundMuxServer<MeteredStream<Q::TcpStream>>,
    <<Q::TcpRoute as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<Q::TcpStream>>,
    <Q::TcpRoute as InboundMuxStreamRoute>::MuxReader: Send,
    Q::QuicRoute: InboundMuxStreamRoute<
            TcpStream = MeteredStream<RecordingStream<Q::QuicStream>>,
            MuxReader = MeteredStream<RecordingStream<Q::QuicStream>>,
        > + Send
        + 'static,
    <Q::QuicRoute as InboundMuxStreamRoute>::UdpRelay:
        zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<Q::QuicStream>>>,
    <Q::QuicRoute as InboundMuxStreamRoute>::MuxServer:
        InboundMuxServer<MeteredStream<Q::QuicStream>>,
    <<Q::QuicRoute as InboundMuxStreamRoute>::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<Q::QuicStream>>,
    <Q::QuicRoute as InboundMuxStreamRoute>::MuxReader: Send,
    <<Q::TcpRoute as InboundMuxStreamRoute>::MuxServer as InboundMuxServer<
        MeteredStream<Q::TcpStream>,
    >>::TcpRelay: From<
        <<Q::QuicRoute as InboundMuxStreamRoute>::MuxServer as InboundMuxServer<
            MeteredStream<Q::QuicStream>,
        >>::TcpRelay,
    >,
    <<Q::TcpRoute as InboundMuxStreamRoute>::MuxServer as InboundMuxServer<
        MeteredStream<Q::TcpStream>,
    >>::UdpRelay: From<
        <<Q::QuicRoute as InboundMuxStreamRoute>::MuxServer as InboundMuxServer<
            MeteredStream<Q::QuicStream>,
        >>::UdpRelay,
    >,
{
    spawn_recorded_transport_mux_bound_inbound_listener_with_request::<
        Q,
        Q::TcpStream,
        Q::TcpRoute,
        Q::TcpFallback,
        Q::QuicStream,
        Q::QuicRoute,
        Q::QuicFallback,
        _,
        _,
        _,
        _,
    >(
        proxy,
        inbound,
        bound,
        shutdown_rx,
        listeners,
        |request, socket| request.accept_tcp_bound_route(socket),
        |request, stream| request.accept_quic_bound_route(stream),
    );
}

pub(crate) fn into_recorded_tcp_relay_stream<S>(
    metered: MeteredStream<RecordingStream<S>>,
) -> TcpRelayStream
where
    S: ClientStream + 'static,
{
    TcpRelayStream::new(metered.into_unrecorded_inner())
}

pub(crate) fn record_metered_inbound_traffic<S>(
    proxy: &Proxy,
    session_id: u64,
    client: &mut MeteredStream<S>,
) where
    S: ClientStream,
{
    proxy.record_session_inbound_traffic(session_id, client.drain_traffic());
}

pub(crate) async fn run_recorded_protocol_stream_udp_relay<S, R>(
    proxy: Proxy,
    session: Session,
    relay: R,
    inbound_tag: String,
    protocol: &'static str,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    R: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<S>>>,
    R::Responder: StreamUdpResponder<MeteredStream<S>>,
{
    let session_id = session.id;
    let record_proxy = proxy.clone();
    run_mapped_protocol_stream_udp_relay(
        &proxy,
        &session,
        relay,
        &inbound_tag,
        protocol,
        move |mut client| {
            record_proxy.record_session_inbound_traffic(session_id, client.drain_traffic());
            MeteredStream::new(client.into_unrecorded_inner())
        },
        Some(record_metered_inbound_traffic::<S>),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_recorded_protocol_mux_session<S, M, FTcp, FTcpFut, FUdp, FUdpFut>(
    proxy: Proxy,
    mut reader: MeteredStream<RecordingStream<S>>,
    mux_server: M,
    inbound_tag: String,
    mux_protocol: &'static str,
    panic_message: &'static str,
    abort_on_end: bool,
    mut spawn_tcp: FTcp,
    mut spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    M: InboundMuxServer<MeteredStream<S>>,
    FTcp: FnMut(Proxy, Session, M::TcpRelay, String) -> FTcpFut + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(Proxy, M::UdpRelay, String) -> FUdpFut + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    record_metered_inbound_traffic(&proxy, 0, &mut reader);
    let client = MeteredStream::new(reader.into_unrecorded_inner());
    run_protocol_mux_session(
        &proxy,
        client,
        mux_server,
        MuxSessionLoop {
            inbound_tag: &inbound_tag,
            protocol: mux_protocol,
            panic_message,
            abort_on_end,
        },
        |proxy, session, relay, inbound_tag| spawn_tcp(proxy, session, relay, inbound_tag),
        |proxy, relay, inbound_tag| spawn_udp(proxy, relay, inbound_tag),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn dispatch_recorded_protocol_mux_route<R, P, S, FTcp, FTcpFut, FUdp, FUdpFut>(
    route: R,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    protocol: P,
    udp_protocol: &'static str,
    mux_protocol: &'static str,
    panic_message: &'static str,
    abort_on_end: bool,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    R: InboundMuxStreamRoute<
        TcpStream = MeteredStream<RecordingStream<S>>,
        MuxReader = MeteredStream<RecordingStream<S>>,
    >,
    R::UdpRelay: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<S>>>,
    R::MuxServer: InboundMuxServer<MeteredStream<S>>,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            Proxy,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
            String,
        ) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    dispatch_protocol_mux_route(
        route,
        MuxRouteBridge {
            proxy,
            inbound_tag,
            source_addr,
            protocol,
            map_tcp_stream: into_recorded_tcp_relay_stream::<S>,
            run_udp: move |proxy: Proxy,
                           session: Session,
                           relay: R::UdpRelay,
                           inbound_tag: String| {
                run_recorded_protocol_stream_udp_relay::<S, _>(
                    proxy,
                    session,
                    relay,
                    inbound_tag,
                    udp_protocol,
                )
            },
            run_mux: move |proxy: Proxy,
                           reader: R::MuxReader,
                           mux_server: R::MuxServer,
                           inbound_tag: String| {
                run_recorded_protocol_mux_session(
                    proxy,
                    reader,
                    mux_server,
                    inbound_tag,
                    mux_protocol,
                    panic_message,
                    abort_on_end,
                    spawn_tcp,
                    spawn_udp,
                )
            },
        },
    )
    .await
}

pub(crate) struct RecordedProtocolMuxRouteDefaults {
    pub(crate) udp_protocol: &'static str,
    pub(crate) mux_protocol: &'static str,
    pub(crate) panic_message: &'static str,
    pub(crate) abort_on_end: bool,
}

fn recorded_protocol_mux_defaults<Q>() -> RecordedProtocolMuxRouteDefaults
where
    Q: RecordedProtocolMuxRouteDispatchMetadata,
{
    RecordedProtocolMuxRouteDefaults {
        udp_protocol: Q::UDP_PROTOCOL,
        mux_protocol: Q::MUX_PROTOCOL,
        panic_message: Q::PANIC_MESSAGE,
        abort_on_end: Q::ABORT_ON_END,
    }
}

pub(crate) async fn dispatch_recorded_protocol_mux_route_with_udp_logger<
    R,
    P,
    S,
    FTcp,
    FTcpFut,
    FUdp,
    FUdpFut,
>(
    route: R,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    protocol: P,
    defaults: RecordedProtocolMuxRouteDefaults,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    R: InboundMuxStreamRoute<
        TcpStream = MeteredStream<RecordingStream<S>>,
        MuxReader = MeteredStream<RecordingStream<S>>,
    >,
    R::UdpRelay: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<S>>>,
    R::MuxServer: InboundMuxServer<MeteredStream<S>>,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            Proxy,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
            String,
        ) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    dispatch_recorded_protocol_mux_route(
        route,
        proxy,
        inbound_tag,
        source_addr,
        protocol,
        defaults.udp_protocol,
        defaults.mux_protocol,
        defaults.panic_message,
        defaults.abort_on_end,
        spawn_tcp,
        spawn_udp,
    )
    .await
}

pub(crate) async fn dispatch_recorded_protocol_mux_route_accept_result<
    R,
    P,
    S,
    FR,
    FTcp,
    FTcpFut,
    FUdp,
    FUdpFut,
>(
    result: RouteAcceptResult<R, FR>,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    protocol: P,
    defaults: RecordedProtocolMuxRouteDefaults,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    R: InboundMuxStreamRoute<
        TcpStream = MeteredStream<RecordingStream<S>>,
        MuxReader = MeteredStream<RecordingStream<S>>,
    >,
    R::UdpRelay: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<S>>>,
    R::MuxServer: InboundMuxServer<MeteredStream<S>>,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FR: FallbackReplayToUpstream + 'static,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            Proxy,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
            String,
        ) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    match result {
        RouteAcceptResult::Route(route) => {
            dispatch_recorded_protocol_mux_route_with_udp_logger(
                route,
                proxy,
                inbound_tag,
                source_addr,
                protocol,
                defaults,
                spawn_tcp,
                spawn_udp,
            )
            .await
        }
        RouteAcceptResult::Fallback(fallback) => {
            crate::runtime::inbound_fallback::relay_recorded_fallback_replay(
                proxy,
                fallback.config,
                fallback.replay,
            )
            .await
        }
    }
}

pub(crate) async fn dispatch_recorded_protocol_mux_tcp_request_result<
    R,
    P,
    S,
    FR,
    FTcp,
    FTcpFut,
    FUdp,
    FUdpFut,
>(
    accept_result: Result<Option<RouteAcceptResult<R, FR>>, EngineError>,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    protocol: P,
    defaults: RecordedProtocolMuxRouteDefaults,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    R: InboundMuxStreamRoute<
        TcpStream = MeteredStream<RecordingStream<S>>,
        MuxReader = MeteredStream<RecordingStream<S>>,
    >,
    R::UdpRelay: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<S>>>,
    R::MuxServer: InboundMuxServer<MeteredStream<S>>,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FR: FallbackReplayToUpstream + 'static,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            Proxy,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
            String,
        ) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    dispatch_optional_recorded_protocol_mux_route_accept_result(
        accept_result?,
        proxy,
        inbound_tag,
        source_addr,
        protocol,
        defaults,
        spawn_tcp,
        spawn_udp,
    )
    .await
}

pub(crate) async fn dispatch_recorded_protocol_mux_stream_request_result<
    R,
    P,
    S,
    FR,
    FTcp,
    FTcpFut,
    FUdp,
    FUdpFut,
>(
    accept_result: Result<RouteAcceptResult<R, FR>, EngineError>,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    protocol: P,
    defaults: RecordedProtocolMuxRouteDefaults,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    R: InboundMuxStreamRoute<
        TcpStream = MeteredStream<RecordingStream<S>>,
        MuxReader = MeteredStream<RecordingStream<S>>,
    >,
    R::UdpRelay: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<S>>>,
    R::MuxServer: InboundMuxServer<MeteredStream<S>>,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FR: FallbackReplayToUpstream + 'static,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            Proxy,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
            String,
        ) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    dispatch_recorded_protocol_mux_route_accept_result(
        accept_result?,
        proxy,
        inbound_tag,
        source_addr,
        protocol,
        defaults,
        spawn_tcp,
        spawn_udp,
    )
    .await
}

pub(crate) async fn dispatch_optional_recorded_protocol_mux_route_accept_result<
    R,
    P,
    S,
    FR,
    FTcp,
    FTcpFut,
    FUdp,
    FUdpFut,
>(
    result: Option<RouteAcceptResult<R, FR>>,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    protocol: P,
    defaults: RecordedProtocolMuxRouteDefaults,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    S: ClientStream + 'static,
    R: InboundMuxStreamRoute<
        TcpStream = MeteredStream<RecordingStream<S>>,
        MuxReader = MeteredStream<RecordingStream<S>>,
    >,
    R::UdpRelay: zero_core::InboundStreamUdpRelay<Stream = MeteredStream<RecordingStream<S>>>,
    R::MuxServer: InboundMuxServer<MeteredStream<S>>,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FR: FallbackReplayToUpstream + 'static,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            Proxy,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
            String,
        ) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    let Some(result) = result else {
        return Ok(());
    };
    dispatch_recorded_protocol_mux_route_accept_result(
        result,
        proxy,
        inbound_tag,
        source_addr,
        protocol,
        defaults,
        spawn_tcp,
        spawn_udp,
    )
    .await
}
