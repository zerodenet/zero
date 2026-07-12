use tokio::io::{AsyncRead, AsyncWrite};
use zero_core::{InboundStreamRoute, Session, StreamUdpResponder};
use zero_engine::EngineError;

use super::dispatch::dispatch_protocol_stream_route;
use super::model::StreamRouteBridge;
use crate::runtime::inbound_protocol::NoClientResponseInboundProtocol;
use crate::runtime::stream_udp::run_mapped_protocol_stream_udp_relay;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

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
