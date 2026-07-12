use core::future::Future;

use zero_core::{InboundStreamRoute, Session};
use zero_engine::EngineError;

use super::model::StreamRouteBridge;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;

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
