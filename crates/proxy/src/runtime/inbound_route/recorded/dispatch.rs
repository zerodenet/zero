use core::future::Future;

use zero_core::{InboundMuxServer, InboundMuxStreamRoute, Session, StreamUdpResponder};
use zero_engine::EngineError;
use zero_transport::inbound_route::{FallbackReplayToUpstream, RouteAcceptResult};

use super::helpers::{
    into_recorded_tcp_relay_stream, run_recorded_protocol_mux_session,
    run_recorded_protocol_stream_udp_relay,
};
use super::model::RecordedProtocolMuxDispatch;
use crate::runtime::tcp_ingress::InboundProtocol;
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream, RecordingStream, TcpRelayStream};

use super::super::mux::{dispatch_protocol_mux_route, MuxRouteBridge};

pub(crate) async fn dispatch_recorded_protocol_mux_route<R, P, S, FTcp, FTcpFut, FUdp, FUdpFut>(
    route: R,
    request: RecordedProtocolMuxDispatch<P>,
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
    let RecordedProtocolMuxDispatch {
        proxy,
        inbound_tag,
        source_addr,
        protocol,
        defaults,
    } = request;
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
                    defaults.udp_protocol,
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
                    defaults,
                    spawn_tcp,
                    spawn_udp,
                )
            },
        },
    )
    .await
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
    request: RecordedProtocolMuxDispatch<P>,
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
    dispatch_recorded_protocol_mux_route(route, request, spawn_tcp, spawn_udp).await
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
    request: RecordedProtocolMuxDispatch<P>,
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
                route, request, spawn_tcp, spawn_udp,
            )
            .await
        }
        RouteAcceptResult::Fallback(fallback) => {
            crate::runtime::inbound_fallback::relay_recorded_fallback_replay(
                request.proxy,
                fallback.config,
                fallback.replay,
            )
            .await
        }
    }
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
    request: RecordedProtocolMuxDispatch<P>,
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
    dispatch_recorded_protocol_mux_route_accept_result(result, request, spawn_tcp, spawn_udp).await
}
