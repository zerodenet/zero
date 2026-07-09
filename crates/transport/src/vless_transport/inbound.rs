use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use zero_config::{
    FallbackConfig, GrpcConfig, H2Config, HttpUpgradeConfig, InboundProtocolConfig,
    InboundRealityConfig, QuicConfig, SplitHttpConfig, TlsConfig, WebSocketConfig,
};
use zero_engine::EngineError;
use zero_platform_tokio::{ClientStream, PrefixedSocket, TcpRelayStream, TokioSocket};

use crate::inbound_route::{
    InboundFallback, OpaqueFallbackReplay, OpaqueMuxRoute, RouteAcceptResult,
};
use crate::inbound_stack::{accept_inbound_stream_stack, InboundStreamStack};
use crate::{http_upgrade, quic, split_http, tls};

fn record_client_stream<S>(stream: S) -> crate::MeteredStream<crate::RecordingStream<S>>
where
    S: ClientStream + 'static,
{
    crate::MeteredStream::new(crate::RecordingStream::new(stream))
}
#[derive(Clone)]
pub struct VlessInboundListenerRequest {
    profile: vless::inbound::VlessInboundProfile,
    transport: OwnedVlessInboundTransportPlan,
    fallback: Option<FallbackConfig>,
}

impl VlessInboundListenerRequest {
    fn new(
        profile: vless::inbound::VlessInboundProfile,
        transport: OwnedVlessInboundTransportPlan,
        fallback: Option<FallbackConfig>,
    ) -> Self {
        Self {
            profile,
            transport,
            fallback,
        }
    }

    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        let InboundProtocolConfig::Vless {
            users,
            reality,
            tls,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            fallback,
            ..
        } = protocol
        else {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vless inbound request received non-vless inbound config",
            )));
        };

        let profile =
            vless::inbound::VlessInboundProfile::from_config_users(users.iter().map(|user| {
                (
                    user.id.as_str(),
                    user.flow.as_deref(),
                    user.credential_id.as_deref(),
                    user.principal_key.as_deref(),
                    user.up_bps,
                    user.down_bps,
                )
            }))
            .map_err(EngineError::from)?;

        let transport = OwnedVlessInboundTransportPlan::from_config_refs(
            source_dir,
            tls.as_deref(),
            reality.as_deref(),
            ws.as_deref(),
            grpc.as_deref(),
            h2.as_deref(),
            http_upgrade.as_deref(),
            split_http.as_deref(),
            fallback.as_deref(),
        )?;

        Ok(Self::new(profile, transport, fallback.as_deref().cloned()))
    }

    async fn accept_tcp_route<S, FWrap>(
        self,
        socket: TokioSocket,
        wrap_stream: FWrap,
    ) -> Result<
        Option<
            RouteAcceptResult<
                OpaqueMuxRoute<vless::inbound::VlessAcceptedClientRoute<S>>,
                OpaqueFallbackReplay<TcpRelayStream>,
            >,
        >,
        EngineError,
    >
    where
        S: ClientStream + zero_core::InboundFallbackCapture<Stream = TcpRelayStream> + 'static,
        FWrap: Fn(TcpRelayStream) -> S + Clone + Send + 'static,
    {
        let Self {
            profile,
            transport,
            fallback,
        } = self;
        transport
            .accept_tcp_route(profile, fallback, socket, wrap_stream)
            .await
    }

    async fn accept_stream_route<T, S, FWrap>(
        self,
        stream: T,
        sni: Option<String>,
        wrap_stream: FWrap,
    ) -> Result<
        RouteAcceptResult<
            OpaqueMuxRoute<vless::inbound::VlessAcceptedClientRoute<S>>,
            OpaqueFallbackReplay<<S as zero_core::InboundFallbackCapture>::Stream>,
        >,
        EngineError,
    >
    where
        T: ClientStream + 'static,
        S: ClientStream + zero_core::InboundFallbackCapture + 'static,
        <S as zero_core::InboundFallbackCapture>::Stream: ClientStream + Send + 'static,
        FWrap: Fn(T) -> S + Clone + Send + 'static,
    {
        let Self {
            profile, fallback, ..
        } = self;
        accept_vless_stream_route(profile, fallback, stream, sni, wrap_stream).await
    }

    async fn accept_recorded_tcp_route(
        self,
        socket: TokioSocket,
    ) -> Result<
        Option<
            RouteAcceptResult<
                OpaqueMuxRoute<
                    vless::inbound::VlessAcceptedClientRoute<
                        crate::MeteredStream<crate::RecordingStream<TcpRelayStream>>,
                    >,
                >,
                OpaqueFallbackReplay<TcpRelayStream>,
            >,
        >,
        EngineError,
    > {
        self.accept_tcp_route(socket, record_client_stream).await
    }

    async fn accept_recorded_stream_route<T>(
        self,
        stream: T,
    ) -> Result<
        RouteAcceptResult<
            OpaqueMuxRoute<
                vless::inbound::VlessAcceptedClientRoute<
                    crate::MeteredStream<crate::RecordingStream<T>>,
                >,
            >,
            OpaqueFallbackReplay<T>,
        >,
        EngineError,
    >
    where
        T: ClientStream + Send + 'static,
    {
        self.accept_stream_route(stream, None, record_client_stream)
            .await
    }
}

impl crate::inbound_route::ProtocolInboundRequestFactory for VlessInboundListenerRequest {
    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        VlessInboundListenerRequest::from_protocol_config(protocol, source_dir)
    }
}

impl crate::inbound_route::ProtocolInboundRequestMetadata for VlessInboundListenerRequest {
    const ERROR_PROTOCOL_NAME: &'static str = "vless";

    fn protocol_name(&self) -> &'static str {
        "vless"
    }
}

impl crate::inbound_route::RecordedProtocolMuxRouteDispatchMetadata
    for VlessInboundListenerRequest
{
    type ResponseProtocol = vless::inbound::VlessInbound;

    const UDP_PROTOCOL: &'static str = "vless_udp";
    const MUX_PROTOCOL: &'static str = "vless_mux";
    const PANIC_MESSAGE: &'static str = "vless mux task panicked";
    const ABORT_ON_END: bool = true;

    fn response_protocol(&self) -> Self::ResponseProtocol {
        vless::inbound::VlessInbound
    }
}

#[cfg(feature = "quic")]
#[async_trait::async_trait]
impl crate::inbound_route::RecordedBoundMuxRouteRequest for VlessInboundListenerRequest {
    type TcpStream = TcpRelayStream;
    type TcpRoute = OpaqueMuxRoute<
        vless::inbound::VlessAcceptedClientRoute<
            crate::MeteredStream<crate::RecordingStream<TcpRelayStream>>,
        >,
    >;
    type TcpFallback = OpaqueFallbackReplay<TcpRelayStream>;
    type QuicStream = crate::quic::QuicStream;
    type QuicRoute = OpaqueMuxRoute<
        vless::inbound::VlessAcceptedClientRoute<
            crate::MeteredStream<crate::RecordingStream<crate::quic::QuicStream>>,
        >,
    >;
    type QuicFallback = OpaqueFallbackReplay<crate::quic::QuicStream>;

    async fn accept_tcp_bound_route(
        self,
        socket: TokioSocket,
    ) -> Result<Option<RouteAcceptResult<Self::TcpRoute, Self::TcpFallback>>, EngineError> {
        self.accept_recorded_tcp_route(socket).await
    }

    async fn accept_quic_bound_route(
        self,
        stream: crate::quic::QuicStream,
    ) -> Result<RouteAcceptResult<Self::QuicRoute, Self::QuicFallback>, EngineError> {
        self.accept_recorded_stream_route(stream).await
    }
}

#[derive(Clone)]
struct OwnedVlessInboundTransportPlan {
    tls_acceptor: Option<tls::TlsAcceptor>,
    reality: Option<vless::reality::VlessRealityServerProfile>,
    ws: Option<WebSocketConfig>,
    grpc: Option<GrpcConfig>,
    h2: Option<H2Config>,
    http_upgrade: Option<HttpUpgradeConfig>,
    split_http: Option<SplitHttpConfig>,
    split_http_registry: Option<split_http::SplitHttpRegistry>,
    fallback_alpn: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct OwnedVlessInboundBindPlan {
    quic_cert_path: Option<String>,
    quic_key_path: Option<String>,
    source_dir: Option<PathBuf>,
}

impl OwnedVlessInboundBindPlan {
    fn from_config_ref(source_dir: Option<&Path>, quic: Option<&QuicConfig>) -> Self {
        Self {
            quic_cert_path: quic.and_then(|quic| quic.cert_path.clone()),
            quic_key_path: quic.and_then(|quic| quic.key_path.clone()),
            source_dir: source_dir.map(PathBuf::from),
        }
    }

    async fn bind(&self, listen_addr: &str) -> Result<Option<quic::QuicInbound>, EngineError> {
        match (
            self.quic_cert_path.as_deref(),
            self.quic_key_path.as_deref(),
        ) {
            (Some(cert_path), Some(key_path)) => Ok(Some(
                quic::QuicInbound::bind(
                    listen_addr,
                    cert_path,
                    key_path,
                    self.source_dir.as_deref(),
                )
                .await?,
            )),
            (None, None) => Ok(None),
            _ => Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vless quic inbound bind requires both cert_path and key_path",
            ))),
        }
    }
}

#[async_trait::async_trait]
impl crate::inbound_route::ProtocolInboundBindPlan for OwnedVlessInboundBindPlan {
    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        match protocol {
            InboundProtocolConfig::Vless { quic, .. } => Ok(Self::from_config_ref(
                source_dir,
                quic.as_ref().map(|quic| quic.as_ref()),
            )),
            _ => Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vless inbound bind received non-vless inbound config",
            ))),
        }
    }

    async fn bind(
        &self,
        listen_addr: &str,
    ) -> Result<crate::inbound_route::TransportInboundBindTarget, EngineError> {
        match OwnedVlessInboundBindPlan::bind(self, listen_addr).await? {
            Some(endpoint) => Ok(crate::inbound_route::TransportInboundBindTarget::Quic(
                endpoint,
            )),
            None => Ok(crate::inbound_route::TransportInboundBindTarget::Tcp),
        }
    }
}

impl OwnedVlessInboundTransportPlan {
    #[allow(clippy::too_many_arguments)]
    fn from_config_refs(
        source_dir: Option<&Path>,
        tls: Option<&TlsConfig>,
        reality: Option<&InboundRealityConfig>,
        ws: Option<&WebSocketConfig>,
        grpc: Option<&GrpcConfig>,
        h2: Option<&H2Config>,
        http_upgrade: Option<&HttpUpgradeConfig>,
        split_http: Option<&SplitHttpConfig>,
        fallback: Option<&FallbackConfig>,
    ) -> Result<Self, EngineError> {
        Ok(Self {
            tls_acceptor: crate::inbound_stack::build_optional_tls_acceptor(source_dir, tls)?,
            reality: reality.map(|reality| {
                vless::reality::VlessRealityServerProfile::from_config_server(
                    reality.private_key.clone(),
                    reality.short_ids.clone(),
                    reality.server_name.clone(),
                    reality.cipher_suites.clone(),
                )
            }),
            ws: ws.cloned(),
            grpc: grpc.cloned(),
            h2: h2.cloned(),
            http_upgrade: http_upgrade.cloned(),
            split_http: split_http.cloned(),
            split_http_registry: split_http.map(|_| split_http::SplitHttpRegistry::new()),
            fallback_alpn: fallback.and_then(|fallback| fallback.alpn.clone()),
        })
    }

    async fn accept_tcp_inbound(
        self,
        socket: TokioSocket,
    ) -> Result<Option<VlessTcpInboundAcceptResult>, EngineError> {
        let Self {
            tls_acceptor,
            reality,
            ws,
            grpc,
            h2,
            http_upgrade,
            split_http,
            split_http_registry,
            fallback_alpn,
        } = self;

        match accept_vless_inbound_transport(socket, tls_acceptor, reality, fallback_alpn).await? {
            VlessInboundTransportResult::FallbackReplay(fallback_replay) => Ok(Some(
                VlessTcpInboundAcceptResult::FallbackReplay(fallback_replay),
            )),
            VlessInboundTransportResult::Stream { stream, sni } => accept_vless_inbound_carrier(
                stream,
                sni,
                ws,
                grpc,
                h2,
                split_http,
                split_http_registry,
                http_upgrade,
            )
            .await
            .map(|accepted| {
                accepted.map(|(stream, sni)| VlessTcpInboundAcceptResult::Stream { stream, sni })
            }),
        }
    }

    async fn accept_tcp_route<S, FWrap>(
        self,
        profile: vless::inbound::VlessInboundProfile,
        fallback: Option<FallbackConfig>,
        socket: TokioSocket,
        wrap_stream: FWrap,
    ) -> Result<
        Option<
            RouteAcceptResult<
                OpaqueMuxRoute<vless::inbound::VlessAcceptedClientRoute<S>>,
                OpaqueFallbackReplay<TcpRelayStream>,
            >,
        >,
        EngineError,
    >
    where
        S: ClientStream + zero_core::InboundFallbackCapture<Stream = TcpRelayStream> + 'static,
        FWrap: Fn(TcpRelayStream) -> S + Clone + Send + 'static,
    {
        let Some(accepted) = self.accept_tcp_inbound(socket).await? else {
            return Ok(None);
        };

        match accepted {
            VlessTcpInboundAcceptResult::Stream { stream, sni } => profile
                .accept_route_owned_with_sni_or_else(
                    vless::inbound::VlessInbound,
                    wrap_stream(stream),
                    sni,
                    |route| async move { Ok(RouteAcceptResult::Route(OpaqueMuxRoute::new(route))) },
                    move |auth_error, fallback_replay| {
                        let fallback = fallback.clone();
                        async move {
                            match fallback {
                                Some(fallback) => {
                                    Ok(RouteAcceptResult::Fallback(InboundFallback {
                                        config: fallback,
                                        replay: OpaqueFallbackReplay::new(move |upstream| {
                                            Box::pin(async move {
                                                vless::inbound::VlessFallbackReplay::replay_to_upstream(
                                                    fallback_replay,
                                                    upstream,
                                                )
                                                .await
                                            })
                                        }),
                                    }))
                                }
                                None => Err(EngineError::Core(auth_error)),
                            }
                        }
                    },
                )
                .await
                .map(Some),
            VlessTcpInboundAcceptResult::FallbackReplay(fallback_replay) => {
                let fallback = fallback.ok_or_else(|| {
                    EngineError::Io(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "fallback replay requires fallback config",
                    ))
                })?;
                Ok(Some(RouteAcceptResult::Fallback(InboundFallback {
                    config: fallback,
                    replay: OpaqueFallbackReplay::new(move |upstream| {
                        Box::pin(async move {
                            vless::inbound::VlessFallbackReplay::replay_to_upstream(
                                fallback_replay,
                                upstream,
                            )
                            .await
                            .map(TcpRelayStream::from)
                        })
                    }),
                })))
            }
        }
    }
}

enum VlessTcpInboundAcceptResult {
    Stream {
        stream: TcpRelayStream,
        sni: Option<String>,
    },
    FallbackReplay(vless::inbound::VlessFallbackReplay<TokioSocket>),
}

async fn accept_vless_stream_route<T, S, FWrap>(
    profile: vless::inbound::VlessInboundProfile,
    fallback: Option<FallbackConfig>,
    stream: T,
    sni: Option<String>,
    wrap_stream: FWrap,
) -> Result<
    RouteAcceptResult<
        OpaqueMuxRoute<vless::inbound::VlessAcceptedClientRoute<S>>,
        OpaqueFallbackReplay<<S as zero_core::InboundFallbackCapture>::Stream>,
    >,
    EngineError,
>
where
    T: ClientStream + 'static,
    S: ClientStream + zero_core::InboundFallbackCapture + 'static,
    <S as zero_core::InboundFallbackCapture>::Stream: ClientStream + Send + 'static,
    FWrap: Fn(T) -> S + Clone + Send + 'static,
{
    profile
        .accept_route_owned_with_sni_or_else(
            vless::inbound::VlessInbound,
            wrap_stream(stream),
            sni,
            |route| async move { Ok(RouteAcceptResult::Route(OpaqueMuxRoute::new(route))) },
            move |auth_error, fallback_replay| {
                let fallback = fallback.clone();
                async move {
                    match fallback {
                        Some(fallback) => Ok(RouteAcceptResult::Fallback(InboundFallback {
                            config: fallback,
                            replay: OpaqueFallbackReplay::new(move |upstream| {
                                Box::pin(async move {
                                    vless::inbound::VlessFallbackReplay::replay_to_upstream(
                                        fallback_replay,
                                        upstream,
                                    )
                                    .await
                                })
                            }),
                        })),
                        None => Err(EngineError::Core(auth_error)),
                    }
                }
            },
        )
        .await
}

enum VlessInboundTransportResult {
    Stream {
        stream: VlessInboundTransportStream,
        sni: Option<String>,
    },
    FallbackReplay(vless::inbound::VlessFallbackReplay<TokioSocket>),
}

enum VlessInboundTransportStream {
    Raw(TokioSocket),
    Tls(Box<tls::InboundTlsStream<PrefixedSocket>>),
    Reality(Box<vless::reality::RealityTlsStream<TokioSocket>>),
}

async fn accept_vless_inbound_transport(
    stream: TokioSocket,
    tls_acceptor: Option<tls::TlsAcceptor>,
    reality: Option<vless::reality::VlessRealityServerProfile>,
    fallback_alpn: Option<String>,
) -> Result<VlessInboundTransportResult, EngineError> {
    match (tls_acceptor.as_ref(), reality.as_ref()) {
        (Some(acceptor), None) => {
            accept_vless_tls_inbound_transport(stream, acceptor, fallback_alpn).await
        }
        (None, Some(profile)) => Ok(VlessInboundTransportResult::Stream {
            stream: VlessInboundTransportStream::Reality(Box::new(
                profile.upgrade_server(stream).await?,
            )),
            sni: None,
        }),
        (None, None) => Ok(VlessInboundTransportResult::Stream {
            stream: VlessInboundTransportStream::Raw(stream),
            sni: None,
        }),
        (Some(_), Some(_)) => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "vless inbound cannot set both tls and reality",
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
async fn accept_vless_inbound_carrier(
    stream: VlessInboundTransportStream,
    sni: Option<String>,
    ws: Option<WebSocketConfig>,
    grpc: Option<GrpcConfig>,
    h2: Option<H2Config>,
    split_http: Option<SplitHttpConfig>,
    split_http_registry: Option<split_http::SplitHttpRegistry>,
    http_upgrade: Option<HttpUpgradeConfig>,
) -> Result<Option<(TcpRelayStream, Option<String>)>, EngineError> {
    if let Some(config) = split_http.as_ref() {
        let registry = split_http_registry.as_ref().ok_or_else(|| {
            EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "vless inbound: split-http registry is required",
            ))
        })?;
        return match split_http::accept_xhttp_inbound(stream, config, registry).await? {
            Some(stream) => Ok(Some((TcpRelayStream::new(stream), sni))),
            None => Ok(None),
        };
    }

    if let Some(config) = http_upgrade.as_ref() {
        let stream = http_upgrade::accept_http_upgrade(stream, config).await?;
        return Ok(Some((TcpRelayStream::new(stream), sni)));
    }

    let clear_sni = grpc.is_some();
    let stream = accept_inbound_stream_stack(
        stream,
        InboundStreamStack {
            ws: ws.as_ref(),
            grpc: grpc.as_ref(),
            h2: h2.as_ref(),
        },
        "vless inbound: ws, grpc, and h2 are mutually exclusive",
    )
    .await?;
    Ok(Some((stream, if clear_sni { None } else { sni })))
}

impl zero_traits::AsyncSocket for VlessInboundTransportStream {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        match self {
            Self::Raw(stream) => zero_traits::AsyncSocket::read(stream, buf).await,
            Self::Tls(stream) => {
                <tls::InboundTlsStream<PrefixedSocket> as zero_traits::AsyncSocket>::read(
                    stream.as_mut(),
                    buf,
                )
                .await
            }
            Self::Reality(stream) => {
                <vless::reality::RealityTlsStream<TokioSocket> as zero_traits::AsyncSocket>::read(
                    stream.as_mut(),
                    buf,
                )
                .await
            }
        }
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        match self {
            Self::Raw(stream) => stream.write_all(buf).await,
            Self::Tls(stream) => stream.write_all(buf).await,
            Self::Reality(stream) => stream.write_all(buf).await,
        }
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        match self {
            Self::Raw(stream) => stream.shutdown().await,
            Self::Tls(stream) => stream.shutdown().await,
            Self::Reality(stream) => stream.shutdown().await,
        }
    }
}

impl AsyncRead for VlessInboundTransportStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.as_mut().get_mut() {
            Self::Raw(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Reality(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for VlessInboundTransportStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.as_mut().get_mut() {
            Self::Raw(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Tls(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Reality(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.as_mut().get_mut() {
            Self::Raw(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tls(stream) => Pin::new(stream).poll_flush(cx),
            Self::Reality(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.as_mut().get_mut() {
            Self::Raw(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tls(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Reality(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

impl ClientStream for VlessInboundTransportStream {
    fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        match self {
            Self::Raw(stream) => stream.local_addr(),
            Self::Tls(stream) => stream.local_addr(),
            Self::Reality(_stream) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "reality inbound local_addr not available",
            )),
        }
    }

    fn peer_addr(&self) -> io::Result<std::net::SocketAddr> {
        match self {
            Self::Raw(stream) => stream.peer_addr(),
            Self::Tls(stream) => stream.peer_addr(),
            Self::Reality(_stream) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "reality inbound peer_addr not available",
            )),
        }
    }
}

async fn accept_vless_tls_inbound_transport(
    mut stream: TokioSocket,
    tls_acceptor: &tls::TlsAcceptor,
    fallback_alpn: Option<String>,
) -> Result<VlessInboundTransportResult, EngineError> {
    let hello = tls::peek_client_hello(&mut stream).await.ok().flatten();

    if let Some(hello) = hello {
        let tls::InboundClientHello {
            sni,
            alpn,
            consumed,
        } = hello;
        if let Some(expected_alpn) = fallback_alpn.as_deref() {
            match vless::inbound::fallback_replay_for_alpns(
                Some(expected_alpn),
                alpn.iter().map(|alpn| alpn.as_str()),
                stream,
                consumed,
            )
            .into_transport_parts()
            {
                Ok(fallback_replay) => {
                    return Ok(VlessInboundTransportResult::FallbackReplay(fallback_replay));
                }
                Err((stream, replay_head)) => {
                    let tls_stream = tls_acceptor
                        .accept(PrefixedSocket::from_prefix(stream, replay_head))
                        .await
                        .map_err(|error| EngineError::Io(io::Error::other(error)))?;
                    return Ok(VlessInboundTransportResult::Stream {
                        stream: VlessInboundTransportStream::Tls(Box::new(
                            tls::InboundTlsStream::new_generic(tls_stream),
                        )),
                        sni,
                    });
                }
            }
        }

        let tls_stream = tls_acceptor
            .accept(PrefixedSocket::from_prefix(stream, consumed))
            .await
            .map_err(|error| EngineError::Io(io::Error::other(error)))?;
        return Ok(VlessInboundTransportResult::Stream {
            stream: VlessInboundTransportStream::Tls(Box::new(tls::InboundTlsStream::new_generic(
                tls_stream,
            ))),
            sni,
        });
    }

    let tls_stream = tls_acceptor
        .accept(PrefixedSocket::from_prefix(stream, Vec::new()))
        .await
        .map_err(|error| EngineError::Io(io::Error::other(error)))?;
    Ok(VlessInboundTransportResult::Stream {
        stream: VlessInboundTransportStream::Tls(Box::new(tls::InboundTlsStream::new_generic(
            tls_stream,
        ))),
        sni: None,
    })
}
