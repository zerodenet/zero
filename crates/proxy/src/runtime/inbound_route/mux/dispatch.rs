use core::future::Future;

use zero_core::{InboundMuxStreamRoute, Session};
use zero_engine::EngineError;

use super::model::MuxRouteBridge;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;

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
