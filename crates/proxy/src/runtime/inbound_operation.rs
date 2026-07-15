use std::future::Future;
use std::pin::Pin;

use tokio::sync::watch;
use zero_engine::EngineError;

use crate::protocol_registry::BoundInbound;
use crate::runtime::route_runtime::InboundRouteRuntime;
use crate::runtime::Proxy;

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

pub(crate) trait PreparedInboundListenerOperation: Send {
    fn execute(
        self: Box<Self>,
        proxy: Proxy,
        bound: BoundInbound,
        shutdown: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EngineError>> + Send + 'static>>;
}

pub(crate) struct TcpInboundListenerOperation<R, D> {
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) error_protocol_name: &'static str,
    pub(crate) request: R,
    pub(crate) dispatch: D,
}

impl<R, D, Fut> PreparedInboundListenerOperation for TcpInboundListenerOperation<R, D>
where
    R: Clone + Send + Sync + 'static,
    D: Fn(R, zero_platform_tokio::TokioSocket, InboundConnectionContext) -> Fut
        + Clone
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = Result<(), EngineError>> + Send + 'static,
{
    fn execute(
        self: Box<Self>,
        proxy: Proxy,
        bound: BoundInbound,
        shutdown: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EngineError>> + Send + 'static>> {
        Box::pin(async move {
            let TcpInboundListenerOperation {
                inbound_tag,
                protocol_name,
                error_protocol_name,
                request,
                dispatch,
            } = *self;
            crate::runtime::listener_loop::run_logged_tcp_socket_listener_loop(
                crate::runtime::listener_loop::LoggedTcpSocketListenerRequest {
                    proxy: &proxy,
                    inbound_tag,
                    protocol_name,
                    error_protocol_name,
                    request,
                    listener: bound.into_tcp(),
                    shutdown,
                    dispatch: move |runtime, request, socket| {
                        let dispatch = dispatch.clone();
                        async move {
                            dispatch(request, socket, InboundConnectionContext::new(runtime)).await
                        }
                    },
                },
            )
            .await
        })
    }
}

#[cfg(feature = "shadowsocks")]
pub(crate) struct TcpAndDatagramInboundListenerOperation<R, D, U> {
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) error_protocol_name: &'static str,
    pub(crate) listen_address: String,
    pub(crate) listen_port: u16,
    pub(crate) tcp_request: R,
    pub(crate) tcp_dispatch: D,
    pub(crate) udp_relay: U,
}

#[cfg(feature = "shadowsocks")]
impl<R, D, Fut, U> PreparedInboundListenerOperation
    for TcpAndDatagramInboundListenerOperation<R, D, U>
where
    R: Clone + Send + Sync + 'static,
    D: Fn(R, zero_platform_tokio::TokioSocket, InboundConnectionContext) -> Fut
        + Clone
        + Send
        + Sync
        + 'static,
    Fut: Future<Output = Result<(), EngineError>> + Send + 'static,
    U: zero_core::InboundDatagramUdpRelay<std::sync::Arc<tokio::net::UdpSocket>> + Send + 'static,
{
    fn execute(
        self: Box<Self>,
        proxy: Proxy,
        bound: BoundInbound,
        shutdown: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EngineError>> + Send + 'static>> {
        Box::pin(async move {
            let TcpAndDatagramInboundListenerOperation {
                inbound_tag,
                protocol_name,
                error_protocol_name,
                listen_address,
                listen_port,
                tcp_request,
                tcp_dispatch,
                udp_relay,
            } = *self;
            let udp_socket = match tokio::net::UdpSocket::bind(format!(
                "{listen_address}:{listen_port}"
            ))
            .await
            {
                Ok(socket) => Some(std::sync::Arc::new(socket)),
                Err(error) => {
                    tracing::warn!(%error, protocol = protocol_name, "failed to bind inbound UDP socket; UDP disabled");
                    None
                }
            };
            let udp_task = udp_socket.as_ref().map(|socket| {
                let runtime = crate::runtime::udp_ingress::UdpIngressRuntime::from_proxy(&proxy);
                let inbound_tag = inbound_tag.clone();
                let socket = socket.clone();
                tokio::spawn(async move {
                    crate::runtime::datagram_udp::run_protocol_datagram_udp_relay(
                        runtime,
                        socket,
                        udp_relay,
                        &inbound_tag,
                        false,
                    )
                    .await
                })
            });

            let result = Box::new(TcpInboundListenerOperation {
                inbound_tag,
                protocol_name,
                error_protocol_name,
                request: tcp_request,
                dispatch: tcp_dispatch,
            })
            .execute(proxy, bound, shutdown)
            .await;

            if let Some(task) = udp_task {
                task.abort();
                let _ = task.await;
            }
            result
        })
    }
}

#[cfg(feature = "hysteria2")]
pub(crate) struct AuthenticatedQuicInboundListenerOperation<P> {
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) profile: P,
}

#[cfg(feature = "hysteria2")]
impl<P> PreparedInboundListenerOperation for AuthenticatedQuicInboundListenerOperation<P>
where
    P: zero_transport::inbound_quic::AuthenticatedQuicInboundProfile,
{
    fn execute(
        self: Box<Self>,
        proxy: Proxy,
        bound: BoundInbound,
        shutdown: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EngineError>> + Send + 'static>> {
        Box::pin(async move {
            let listener = match bound {
                BoundInbound::Quic(listener) => listener,
                BoundInbound::Tcp(_) => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "authenticated QUIC inbound received a TCP listener",
                    )))
                }
            };
            let profile = self.profile;
            let protocol_name = self.protocol_name;
            crate::runtime::listener_loop::run_quic_listener_loop(
                crate::runtime::listener_loop::QuicListenerLoopRequest {
                    proxy: &proxy,
                    inbound_tag: self.inbound_tag,
                    protocol_name,
                    listener,
                    shutdown,
                    handler: move |runtime, connection| {
                        let profile = profile.clone();
                        async move {
                            if let Err(error) = run_authenticated_quic_connection(
                                profile,
                                runtime,
                                connection,
                            )
                            .await
                            {
                                tracing::error!(%error, protocol = protocol_name, "inbound QUIC connection failed");
                            }
                        }
                    },
                },
            )
            .await
        })
    }
}

#[cfg(feature = "hysteria2")]
async fn run_authenticated_quic_connection<P>(
    profile: P,
    runtime: InboundRouteRuntime,
    connection: quinn::Connection,
) -> Result<(), EngineError>
where
    P: zero_transport::inbound_quic::AuthenticatedQuicInboundProfile,
{
    use zero_transport::inbound_quic::AuthenticatedQuicInboundConnection;

    let connection = profile.accept_authenticated_connection(connection).await?;
    let mut tasks = tokio::task::JoinSet::new();
    let udp_source = connection.datagram_source();
    let udp_relay = connection.udp_relay();
    let udp_runtime = runtime.udp_runtime();
    let udp_tag = runtime.inbound_tag().to_owned();
    tasks.spawn(async move {
        crate::runtime::datagram_udp::run_protocol_datagram_udp_relay(
            udp_runtime,
            udp_source,
            udp_relay,
            &udp_tag,
            false,
        )
        .await
    });

    loop {
        tokio::select! {
            accepted = connection.accept_next_tcp_stream() => {
                let Some((session, stream)) = accepted? else {
                    break;
                };
                let context = InboundConnectionContext::new(runtime.clone());
                let response = connection.response_protocol();
                tasks.spawn(async move {
                    context.serve_with_client_response(session, stream, response).await
                });
            }
            result = tasks.join_next(), if !tasks.is_empty() => {
                match result {
                    Some(Ok(Ok(()))) => {}
                    Some(Ok(Err(error))) => tracing::warn!(%error, "inbound QUIC stream task failed"),
                    Some(Err(error)) if !error.is_cancelled() => {
                        tracing::error!(%error, "inbound QUIC stream task panicked");
                    }
                    Some(Err(_)) | None => {}
                }
            }
        }
    }

    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
    Ok(())
}

#[cfg(feature = "vless")]
pub(crate) struct TcpOrQuicInboundListenerOperation<R, TD, QD> {
    pub(crate) inbound_tag: String,
    pub(crate) protocol_name: &'static str,
    pub(crate) error_protocol_name: &'static str,
    pub(crate) request: R,
    pub(crate) dispatch_tcp: TD,
    pub(crate) dispatch_quic: QD,
}

#[cfg(feature = "vless")]
impl<R, TD, TFut, QD, QFut> PreparedInboundListenerOperation
    for TcpOrQuicInboundListenerOperation<R, TD, QD>
where
    R: Clone + Send + Sync + 'static,
    TD: Fn(R, zero_platform_tokio::TokioSocket, InboundConnectionContext) -> TFut
        + Clone
        + Send
        + Sync
        + 'static,
    TFut: Future<Output = Result<(), EngineError>> + Send + 'static,
    QD: Fn(R, crate::transport::QuicStream, InboundConnectionContext) -> QFut
        + Clone
        + Send
        + Sync
        + 'static,
    QFut: Future<Output = Result<(), EngineError>> + Send + 'static,
{
    fn execute(
        self: Box<Self>,
        proxy: Proxy,
        bound: BoundInbound,
        shutdown: watch::Receiver<bool>,
    ) -> Pin<Box<dyn Future<Output = Result<(), EngineError>> + Send + 'static>> {
        Box::pin(async move {
            let TcpOrQuicInboundListenerOperation {
                inbound_tag,
                protocol_name,
                error_protocol_name,
                request,
                dispatch_tcp,
                dispatch_quic,
            } = *self;
            match bound {
                BoundInbound::Tcp(listener) => {
                    crate::runtime::listener_loop::run_logged_tcp_socket_listener_loop(
                        crate::runtime::listener_loop::LoggedTcpSocketListenerRequest {
                            proxy: &proxy,
                            inbound_tag,
                            protocol_name,
                            error_protocol_name,
                            request,
                            listener,
                            shutdown,
                            dispatch: move |runtime, request, socket| {
                                let dispatch = dispatch_tcp.clone();
                                async move {
                                    dispatch(
                                        request,
                                        socket,
                                        InboundConnectionContext::new(runtime),
                                    )
                                    .await
                                }
                            },
                        },
                    )
                    .await
                }
                BoundInbound::Quic(listener) => {
                    crate::runtime::listener_loop::run_logged_quic_stream_listener_loop(
                        crate::runtime::listener_loop::LoggedQuicStreamListenerRequest {
                            proxy: &proxy,
                            inbound_tag,
                            protocol_name,
                            error_protocol_name,
                            request,
                            listener,
                            shutdown,
                            dispatch: move |runtime, request, stream| {
                                let dispatch = dispatch_quic.clone();
                                async move {
                                    dispatch(
                                        request,
                                        stream,
                                        InboundConnectionContext::new(runtime),
                                    )
                                    .await
                                }
                            },
                        },
                    )
                    .await
                }
            }
        })
    }
}
