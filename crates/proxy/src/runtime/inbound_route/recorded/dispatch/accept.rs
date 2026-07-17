use core::future::Future;

use zero_core::{InboundMuxServer, InboundMuxStreamRoute, Session, StreamUdpResponder};
use zero_engine::EngineError;
use zero_transport::protocol_inbound_route::{FallbackReplayToUpstream, RouteAcceptResult};

use super::route::dispatch_recorded_protocol_mux_route_with_udp_logger;
use crate::runtime::route_runtime::MuxSubstreamRuntime;
use crate::runtime::tcp_ingress::InboundProtocol;
use crate::transport::{ClientStream, MeteredStream, RecordingStream, TcpRelayStream};

use crate::runtime::inbound_route::recorded::model::RecordedProtocolMuxDispatch;

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
            MuxSubstreamRuntime,
            Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            MuxSubstreamRuntime,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
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
            request
                .runtime
                .relay_recorded_fallback_replay(fallback.config, fallback.replay)
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
            MuxSubstreamRuntime,
            Session,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::TcpRelay,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(
            MuxSubstreamRuntime,
            <R::MuxServer as InboundMuxServer<MeteredStream<S>>>::UdpRelay,
        ) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    let Some(result) = result else {
        return Ok(());
    };
    dispatch_recorded_protocol_mux_route_accept_result(result, request, spawn_tcp, spawn_udp).await
}
