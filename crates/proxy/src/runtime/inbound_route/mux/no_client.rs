use core::future::Future;

use tokio::io::{AsyncRead, AsyncWrite};
use tracing::warn;
use zero_core::{
    InboundMuxServer, InboundMuxStreamRoute, InboundMuxTcpRelay, InboundMuxUdpRelay, Session,
    StreamUdpResponder,
};
use zero_engine::EngineError;

use super::dispatch::dispatch_protocol_mux_route;
use super::model::{MuxRouteBridge, NoClientMuxRouteDefaults};
use crate::runtime::mux_session::{run_protocol_mux_session, MuxSessionLoop};
use crate::runtime::mux_tcp::run_protocol_mux_tcp_task;
use crate::runtime::mux_udp::run_protocol_mux_udp_task;
use crate::runtime::stream_udp::run_mapped_protocol_stream_udp_relay;
use crate::runtime::tcp_ingress::NoClientResponseInboundProtocol;
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

pub(crate) async fn dispatch_no_client_mux_route<R, FTcp, FTcpFut, FUdp, FUdpFut>(
    route: R,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    defaults: NoClientMuxRouteDefaults,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    R: InboundMuxStreamRoute,
    R::TcpStream: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    R::UdpRelay: zero_core::InboundStreamUdpRelay,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    R::MuxServer: InboundMuxServer<R::MuxReader>,
    R::MuxReader: Send,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<R::MuxReader>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(Proxy, <R::MuxServer as InboundMuxServer<R::MuxReader>>::UdpRelay, String) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    dispatch_protocol_mux_route(
        route,
        MuxRouteBridge {
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
                    crate::runtime::udp_ingress::UdpIngressRuntime::from_proxy(&proxy),
                    &session,
                    relay,
                    &inbound_tag,
                    defaults.udp_protocol,
                    TcpRelayStream::new,
                    None,
                )
                .await
            },
            run_mux: move |proxy: Proxy,
                           reader: R::MuxReader,
                           mux_server: R::MuxServer,
                           inbound_tag: String| async move {
                match run_protocol_mux_session(
                    &proxy,
                    reader,
                    mux_server,
                    MuxSessionLoop {
                        inbound_tag: &inbound_tag,
                        protocol: defaults.mux_protocol,
                        panic_message: defaults.panic_message,
                        abort_on_end: defaults.abort_on_end,
                    },
                    spawn_tcp,
                    spawn_udp,
                )
                .await
                {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        warn!(error = %error, "{}", defaults.read_error_log);
                        Ok(())
                    }
                }
            },
        },
    )
    .await
}

pub(crate) async fn dispatch_no_client_mux_route_with_defaults<R, FTcp, FTcpFut, FUdp, FUdpFut>(
    route: R,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    defaults: NoClientMuxRouteDefaults,
    spawn_tcp: FTcp,
    spawn_udp: FUdp,
) -> Result<(), EngineError>
where
    R: InboundMuxStreamRoute,
    R::TcpStream: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    R::UdpRelay: zero_core::InboundStreamUdpRelay,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    R::MuxServer: InboundMuxServer<R::MuxReader>,
    R::MuxReader: Send,
    FTcp: FnMut(
            Proxy,
            Session,
            <R::MuxServer as InboundMuxServer<R::MuxReader>>::TcpRelay,
            String,
        ) -> FTcpFut
        + Send,
    FTcpFut: Future<Output = ()> + Send + 'static,
    FUdp: FnMut(Proxy, <R::MuxServer as InboundMuxServer<R::MuxReader>>::UdpRelay, String) -> FUdpFut
        + Send,
    FUdpFut: Future<Output = ()> + Send + 'static,
{
    dispatch_no_client_mux_route(
        route,
        proxy,
        inbound_tag,
        source_addr,
        defaults,
        spawn_tcp,
        spawn_udp,
    )
    .await
}

pub(crate) async fn dispatch_no_client_mux_route_request_with_defaults<R>(
    route: R,
    proxy: Proxy,
    inbound_tag: String,
    source_addr: Option<std::net::SocketAddr>,
    defaults: NoClientMuxRouteDefaults,
) -> Result<(), EngineError>
where
    R: InboundMuxStreamRoute,
    R::TcpStream: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    R::UdpRelay: zero_core::InboundStreamUdpRelay,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
        AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
    <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
        StreamUdpResponder<TcpRelayStream>,
    R::MuxServer: InboundMuxServer<R::MuxReader>,
    R::MuxReader: Send,
    <R::MuxServer as InboundMuxServer<R::MuxReader>>::TcpRelay: InboundMuxTcpRelay + 'static,
    <R::MuxServer as InboundMuxServer<R::MuxReader>>::UdpRelay: InboundMuxUdpRelay + 'static,
{
    dispatch_no_client_mux_route_with_defaults(
        route,
        proxy,
        inbound_tag,
        source_addr,
        defaults,
        move |proxy, session, relay, inbound_tag| {
            run_protocol_mux_tcp_task(proxy, session, relay, inbound_tag, defaults.mux_protocol)
        },
        move |proxy, relay, inbound_tag| {
            run_protocol_mux_udp_task(proxy, relay, inbound_tag, defaults.udp_protocol)
        },
    )
    .await
}
