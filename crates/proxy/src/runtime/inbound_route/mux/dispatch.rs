use core::future::Future;

use zero_core::{InboundMuxStreamRoute, Session};
use zero_engine::EngineError;

use super::model::MuxRouteBridge;
use crate::runtime::route_runtime::{InboundRouteRuntime, MuxSubstreamRuntime};
use crate::runtime::tcp_ingress::InboundProtocol;

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
    FRunUdp: FnOnce(InboundRouteRuntime, Session, R::UdpRelay) -> FUdp + Send,
    FUdp: Future<Output = Result<(), EngineError>> + Send,
    FRunMux: FnOnce(MuxSubstreamRuntime, R::MuxReader, R::MuxServer) -> FMux + Send,
    FMux: Future<Output = Result<(), EngineError>> + Send,
{
    let MuxRouteBridge {
        runtime,
        protocol,
        map_tcp_stream,
        run_udp,
        run_mux,
    } = request;

    let tcp_runtime = runtime.clone();
    let udp_runtime = runtime.clone();
    let mux_runtime = runtime.into_mux_substream_runtime();

    route
        .dispatch_inbound_route(
            move |session, stream| async move {
                tcp_runtime
                    .serve(session, map_tcp_stream(stream), &protocol)
                    .await
            },
            move |session, relay| run_udp(udp_runtime, session, relay),
            move |reader, mux_server| run_mux(mux_runtime, reader, mux_server),
        )
        .await
}
