use tokio::io::{AsyncRead, AsyncWrite};
use zero_core::{InboundMuxServer, InboundMuxStreamRoute, InboundMuxTcpRelay, InboundMuxUdpRelay};
use zero_engine::EngineError;

use super::route::dispatch_no_client_mux_route_with_defaults;
use crate::runtime::inbound_route::mux::model::NoClientMuxRouteDefaults;
use crate::runtime::mux_tcp::run_protocol_mux_tcp_task;
use crate::runtime::mux_udp::run_protocol_mux_udp_task;
use crate::runtime::route_runtime::InboundRouteRuntime;
use crate::transport::TcpRelayStream;

pub(crate) async fn dispatch_no_client_mux_route_request_with_defaults<R>(
    route: R,
    runtime: InboundRouteRuntime,
    defaults: NoClientMuxRouteDefaults,
) -> Result<(), EngineError>
where
    R: InboundMuxStreamRoute,
    R::TcpStream: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    R::UdpRelay: zero_core::InboundStreamUdpRelay,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        zero_core::StreamUdpResponder<TcpRelayStream>,
    R::MuxServer: InboundMuxServer<R::MuxReader>,
    R::MuxReader: Send,
    <R::MuxServer as InboundMuxServer<R::MuxReader>>::TcpRelay: InboundMuxTcpRelay + 'static,
    <R::MuxServer as InboundMuxServer<R::MuxReader>>::UdpRelay: InboundMuxUdpRelay + 'static,
{
    dispatch_no_client_mux_route_with_defaults(
        route,
        runtime,
        defaults,
        move |runtime, session, relay| {
            run_protocol_mux_tcp_task(runtime, session, relay, defaults.mux_protocol)
        },
        move |runtime, relay| run_protocol_mux_udp_task(runtime, relay, defaults.udp_protocol),
    )
    .await
}
