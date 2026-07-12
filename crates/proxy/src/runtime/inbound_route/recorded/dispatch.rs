use core::future::Future;

use zero_core::{InboundMuxServer, InboundMuxStreamRoute, Session, StreamUdpResponder};
use zero_engine::EngineError;
use zero_transport::inbound_route::{FallbackReplayToUpstream, RouteAcceptResult};

use super::helpers::{
    into_recorded_tcp_relay_stream, run_recorded_protocol_mux_session,
    run_recorded_protocol_stream_udp_relay,
};
use super::model::RecordedProtocolMuxRouteDefaults;
use crate::runtime::inbound_protocol::InboundProtocol;
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream, RecordingStream, TcpRelayStream};

use super::super::mux::{dispatch_protocol_mux_route, MuxRouteBridge};

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
