use zero_core::{InboundMuxServer, InboundMuxStreamRoute, InboundMuxTcpRelay, InboundMuxUdpRelay};
use zero_engine::EngineError;
use zero_transport::inbound_route::{FallbackReplayToUpstream, RouteAcceptResult};

use super::dispatch::{
    dispatch_optional_recorded_protocol_mux_route_accept_result,
    dispatch_recorded_protocol_mux_route_accept_result,
};
use super::model::{RecordedProtocolMuxDispatch, RecordedProtocolMuxRouteDefaults};
use crate::runtime::mux_tcp::run_protocol_mux_tcp_task;
use crate::runtime::mux_udp::run_protocol_mux_udp_task_with_accept_log;
use crate::runtime::route_runtime::{InboundRouteRuntime, MuxSubstreamRuntime};
use crate::runtime::tcp_ingress::InboundProtocol;
use crate::transport::{ClientStream, MeteredStream, RecordingStream, TcpRelayStream};

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
        zero_core::StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FR: FallbackReplayToUpstream + 'static,
    FTcp: FnMut(
            MuxSubstreamRuntime,
            zero_core::Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
        ) -> FTcpFut
        + Send,
    FTcpFut: core::future::Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            MuxSubstreamRuntime,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
        ) -> FUdpFut
        + Send,
    FUdpFut: core::future::Future<Output = ()> + Send + 'static,
{
    dispatch_optional_recorded_protocol_mux_route_accept_result(
        accept_result?,
        request,
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
        zero_core::StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FR: FallbackReplayToUpstream + 'static,
    FTcp: FnMut(
            MuxSubstreamRuntime,
            zero_core::Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
        ) -> FTcpFut
        + Send,
    FTcpFut: core::future::Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            MuxSubstreamRuntime,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
        ) -> FUdpFut
        + Send,
    FUdpFut: core::future::Future<Output = ()> + Send + 'static,
{
    dispatch_recorded_protocol_mux_route_accept_result(
        accept_result?,
        request,
        spawn_tcp,
        spawn_udp,
    )
    .await
}

pub(crate) async fn dispatch_recorded_protocol_mux_tcp_request_with_defaults<R, P, S, FR>(
    accept_result: Result<Option<RouteAcceptResult<R, FR>>, EngineError>,
    runtime: InboundRouteRuntime,
    protocol: P,
    defaults: RecordedProtocolMuxRouteDefaults,
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
        zero_core::StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FR: FallbackReplayToUpstream + 'static,
    <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay: InboundMuxTcpRelay + 'static,
    <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay: InboundMuxUdpRelay + 'static,
{
    dispatch_recorded_protocol_mux_tcp_request_result(
        accept_result,
        RecordedProtocolMuxDispatch {
            runtime,
            protocol,
            defaults,
        },
        move |runtime, session, relay| {
            run_protocol_mux_tcp_task(runtime, session, relay, defaults.mux_protocol)
        },
        move |runtime, relay| {
            run_protocol_mux_udp_task_with_accept_log(
                runtime,
                relay,
                defaults.udp_protocol,
                defaults.udp_accept_log_message,
            )
        },
    )
    .await
}

pub(crate) async fn dispatch_recorded_protocol_mux_stream_request_with_defaults<R, P, S, FR>(
    accept_result: Result<RouteAcceptResult<R, FR>, EngineError>,
    runtime: InboundRouteRuntime,
    protocol: P,
    defaults: RecordedProtocolMuxRouteDefaults,
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
        zero_core::StreamUdpResponder<MeteredStream<S>>,
    R::MuxReader: Send,
    P: InboundProtocol<ClientStream = TcpRelayStream> + 'static,
    FR: FallbackReplayToUpstream + 'static,
    <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay: InboundMuxTcpRelay + 'static,
    <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay: InboundMuxUdpRelay + 'static,
{
    dispatch_recorded_protocol_mux_stream_request_result(
        accept_result,
        RecordedProtocolMuxDispatch {
            runtime,
            protocol,
            defaults,
        },
        move |runtime, session, relay| {
            run_protocol_mux_tcp_task(runtime, session, relay, defaults.mux_protocol)
        },
        move |runtime, relay| {
            run_protocol_mux_udp_task_with_accept_log(
                runtime,
                relay,
                defaults.udp_protocol,
                defaults.udp_accept_log_message,
            )
        },
    )
    .await
}
