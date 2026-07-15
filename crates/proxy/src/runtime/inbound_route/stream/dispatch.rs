use core::future::Future;

use zero_core::{InboundStreamRoute, Session};
use zero_engine::EngineError;

use super::model::StreamRouteBridge;
use crate::runtime::route_runtime::InboundRouteRuntime;
use crate::runtime::tcp_ingress::InboundProtocol;

pub(crate) async fn dispatch_protocol_stream_route<R, P, FMapTcp, FRunUdp, FUdp>(
    route: R,
    request: StreamRouteBridge<P, FMapTcp, FRunUdp>,
) -> Result<(), EngineError>
where
    R: InboundStreamRoute,
    R::TcpStream: Send,
    P: InboundProtocol + 'static,
    FMapTcp: FnOnce(R::TcpStream) -> P::ClientStream + Send,
    FRunUdp: FnOnce(InboundRouteRuntime, Session, R::UdpRelay) -> FUdp + Send,
    FUdp: Future<Output = Result<(), EngineError>> + Send,
{
    let StreamRouteBridge {
        runtime,
        protocol,
        map_tcp_stream,
        run_udp,
    } = request;

    let tcp_runtime = runtime.clone();
    let udp_runtime = runtime;

    route
        .dispatch_inbound_route(
            move |session, stream| async move {
                tcp_runtime
                    .serve(session, map_tcp_stream(stream), &protocol)
                    .await
            },
            move |session, relay| run_udp(udp_runtime, session, relay),
        )
        .await
}
