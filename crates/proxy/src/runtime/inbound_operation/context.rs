use zero_engine::EngineError;

use crate::runtime::route_runtime::InboundRouteRuntime;

#[derive(Clone)]
pub(crate) struct InboundConnectionContext {
    runtime: InboundRouteRuntime,
}

impl InboundConnectionContext {
    pub(crate) fn new(runtime: InboundRouteRuntime) -> Self {
        Self { runtime }
    }

    #[cfg(feature = "socks5")]
    pub(crate) async fn run_udp_association<S, H>(
        self,
        mut client: crate::transport::MeteredStream<S>,
        relay: zero_platform_tokio::TokioDatagramSocket,
        pending_control_traffic: crate::transport::StreamTraffic,
        handler: H,
    ) -> Result<(), EngineError>
    where
        S: crate::transport::ClientStream,
        H: zero_core::InboundUdpAssociation + zero_core::InboundUdpAssociationResponder,
    {
        let runtime = self.runtime;
        let inbound_tag = runtime.inbound_tag().to_owned();
        crate::runtime::udp_association::run_udp_association_loop(
            crate::runtime::udp_association::UdpAssociationLoopRequest {
                runtime: runtime.udp_runtime(),
                client: &mut client,
                inbound_tag: &inbound_tag,
                relay,
                pending_control_traffic,
                handler,
            },
        )
        .await
    }

    pub(crate) async fn serve<P>(
        self,
        session: zero_core::Session,
        client: P::ClientStream,
        protocol: P,
    ) -> Result<(), EngineError>
    where
        P: crate::runtime::tcp_ingress::InboundProtocol + 'static,
    {
        self.runtime.serve(session, client, &protocol).await
    }

    #[cfg(feature = "http")]
    pub(crate) fn select_http_redirect(
        &self,
        session: &zero_core::Session,
    ) -> Option<(u16, String)> {
        self.runtime.select_http_redirect(session)
    }

    #[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
    pub(crate) async fn serve_with_client_response<P, S>(
        self,
        session: zero_core::Session,
        client: S,
        response_protocol: P,
    ) -> Result<(), EngineError>
    where
        P: zero_core::InboundClientResponse<S> + Send + Sync,
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + zero_traits::AsyncSocket + Unpin + Send,
    {
        self.runtime
            .serve_with_client_response(session, client, response_protocol)
            .await
    }

    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) async fn run_stream_udp_relay<R>(
        self,
        session: zero_core::Session,
        relay: R,
        protocol: &'static str,
    ) -> Result<(), EngineError>
    where
        R: zero_core::InboundStreamUdpRelay,
        R::Stream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
        R::Responder: zero_core::StreamUdpResponder<R::Stream>,
    {
        let runtime = self.runtime;
        let inbound_tag = runtime.inbound_tag().to_owned();
        crate::runtime::stream_udp::run_mapped_protocol_stream_udp_relay(
            runtime.udp_runtime(),
            &session,
            relay,
            &inbound_tag,
            protocol,
            core::convert::identity,
            None,
        )
        .await
    }

    #[cfg(feature = "trojan")]
    pub(crate) async fn dispatch_no_client_stream_route<R>(
        self,
        route: R,
        udp_protocol: &'static str,
    ) -> Result<(), EngineError>
    where
        R: zero_core::InboundStreamRoute,
        R::TcpStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
        R::UdpRelay: zero_core::InboundStreamUdpRelay,
        <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
            tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
        <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
            zero_core::StreamUdpResponder<crate::transport::TcpRelayStream>,
    {
        crate::runtime::inbound_route::dispatch_no_client_stream_route(
            route,
            self.runtime,
            udp_protocol,
        )
        .await
    }

    #[cfg(feature = "vmess")]
    pub(crate) async fn dispatch_no_client_mux_route<R>(
        self,
        route: R,
        defaults: crate::runtime::inbound_route::NoClientMuxRouteDefaults,
    ) -> Result<(), EngineError>
    where
        R: zero_core::InboundMuxStreamRoute,
        R::TcpStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
        R::UdpRelay: zero_core::InboundStreamUdpRelay,
        <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Stream:
            tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
        <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
            zero_core::StreamUdpResponder<crate::transport::TcpRelayStream>,
        R::MuxServer: zero_core::InboundMuxServer<R::MuxReader>,
        R::MuxReader: Send,
        <R::MuxServer as zero_core::InboundMuxServer<R::MuxReader>>::TcpRelay:
            zero_core::InboundMuxTcpRelay + 'static,
        <R::MuxServer as zero_core::InboundMuxServer<R::MuxReader>>::UdpRelay:
            zero_core::InboundMuxUdpRelay + 'static,
    {
        crate::runtime::inbound_route::dispatch_no_client_mux_route_request_with_defaults(
            route,
            self.runtime,
            defaults,
        )
        .await
    }

    #[cfg(feature = "vless")]
    pub(crate) async fn dispatch_recorded_mux_tcp_route<R, P, S, FR>(
        self,
        accept_result: Result<
            Option<zero_transport::inbound_route::RouteAcceptResult<R, FR>>,
            EngineError,
        >,
        protocol: P,
        defaults: crate::runtime::inbound_route::RecordedProtocolMuxRouteDefaults,
    ) -> Result<(), EngineError>
    where
        S: crate::transport::ClientStream + 'static,
        R: zero_core::InboundMuxStreamRoute<
            TcpStream = crate::transport::MeteredStream<crate::transport::RecordingStream<S>>,
            MuxReader = crate::transport::MeteredStream<crate::transport::RecordingStream<S>>,
        >,
        R::UdpRelay: zero_core::InboundStreamUdpRelay<
            Stream = crate::transport::MeteredStream<crate::transport::RecordingStream<S>>,
        >,
        R::MuxServer: zero_core::InboundMuxServer<crate::transport::MeteredStream<S>>,
        <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
            zero_core::StreamUdpResponder<crate::transport::MeteredStream<S>>,
        R::MuxReader: Send,
        P: crate::runtime::tcp_ingress::InboundProtocol<
                ClientStream = crate::transport::TcpRelayStream,
            > + 'static,
        FR: zero_transport::inbound_route::FallbackReplayToUpstream + 'static,
        <R::MuxServer as zero_core::InboundMuxServer<crate::transport::MeteredStream<S>>>::TcpRelay:
            zero_core::InboundMuxTcpRelay + 'static,
        <R::MuxServer as zero_core::InboundMuxServer<crate::transport::MeteredStream<S>>>::UdpRelay:
            zero_core::InboundMuxUdpRelay + 'static,
    {
        crate::runtime::inbound_route::dispatch_recorded_protocol_mux_tcp_request_with_defaults(
            accept_result,
            self.runtime,
            protocol,
            defaults,
        )
        .await
    }

    #[cfg(feature = "vless")]
    pub(crate) async fn dispatch_recorded_mux_stream_route<R, P, S, FR>(
        self,
        accept_result: Result<zero_transport::inbound_route::RouteAcceptResult<R, FR>, EngineError>,
        protocol: P,
        defaults: crate::runtime::inbound_route::RecordedProtocolMuxRouteDefaults,
    ) -> Result<(), EngineError>
    where
        S: crate::transport::ClientStream + 'static,
        R: zero_core::InboundMuxStreamRoute<
            TcpStream = crate::transport::MeteredStream<crate::transport::RecordingStream<S>>,
            MuxReader = crate::transport::MeteredStream<crate::transport::RecordingStream<S>>,
        >,
        R::UdpRelay: zero_core::InboundStreamUdpRelay<
            Stream = crate::transport::MeteredStream<crate::transport::RecordingStream<S>>,
        >,
        R::MuxServer: zero_core::InboundMuxServer<crate::transport::MeteredStream<S>>,
        <R::UdpRelay as zero_core::InboundStreamUdpRelay>::Responder:
            zero_core::StreamUdpResponder<crate::transport::MeteredStream<S>>,
        R::MuxReader: Send,
        P: crate::runtime::tcp_ingress::InboundProtocol<
                ClientStream = crate::transport::TcpRelayStream,
            > + 'static,
        FR: zero_transport::inbound_route::FallbackReplayToUpstream + 'static,
        <R::MuxServer as zero_core::InboundMuxServer<crate::transport::MeteredStream<S>>>::TcpRelay:
            zero_core::InboundMuxTcpRelay + 'static,
        <R::MuxServer as zero_core::InboundMuxServer<crate::transport::MeteredStream<S>>>::UdpRelay:
            zero_core::InboundMuxUdpRelay + 'static,
    {
        crate::runtime::inbound_route::dispatch_recorded_protocol_mux_stream_request_with_defaults(
            accept_result,
            self.runtime,
            protocol,
            defaults,
        )
        .await
    }
}
