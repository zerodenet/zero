use std::io;
use std::path::Path;

use zero_config::{
    FallbackConfig, GrpcConfig, H2Config, HttpUpgradeConfig, InboundRealityConfig, SplitHttpConfig,
    TlsConfig, WebSocketConfig,
};
use zero_engine::EngineError;
use zero_platform_tokio::{ClientStream, TcpRelayStream, TokioSocket};

use crate::inbound_route::{
    InboundFallback, OpaqueFallbackReplay, OpaqueMuxRoute, RouteAcceptResult,
};
use crate::{split_http, tls};

use super::carrier::{
    accept_vless_inbound_carrier, accept_vless_inbound_transport, VlessInboundTransportResult,
};

#[derive(Clone)]
pub(super) struct OwnedVlessInboundTransportPlan {
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

impl OwnedVlessInboundTransportPlan {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_config_refs(
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

    pub(super) async fn accept_tcp_route<S, FWrap>(
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

pub(super) async fn accept_vless_stream_route<T, S, FWrap>(
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
