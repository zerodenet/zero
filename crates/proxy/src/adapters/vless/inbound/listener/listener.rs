use crate::logging::log_listener_connection_error;
use crate::runtime::listener_loop::{
    run_quic_stream_listener_loop, run_tcp_listener_loop, QuicStreamListenerLoopRequest,
    TcpListenerLoopRequest,
};
use crate::runtime::Proxy;
use crate::transport::{InboundTlsStream, PrefixedSocket};
use tokio::sync::watch;
use zero_engine::EngineError;
use zero_platform_tokio::TokioSocket;

use super::fallback::relay_fallback;
use super::model::VlessInboundRequest;
use super::session::{handle_vless_client, handle_vless_stream};
use super::upgrade_vless_reality_server;

pub(crate) async fn run_vless_listener_with_bound(
    proxy: &Proxy,
    request: VlessInboundRequest,
    bound: crate::protocol_registry::BoundInbound,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let VlessInboundRequest {
        inbound,
        profile,
        reality,
        tls_acceptor,
        ws,
        grpc,
        h2,
        http_upgrade,
        split_http,
        fallback,
    } = request;

    match bound {
        crate::protocol_registry::BoundInbound::Quic(quic_inbound) => {
            let fallback_config = fallback.as_deref().cloned();
            return run_quic_stream_listener_loop(QuicStreamListenerLoopRequest {
                proxy,
                inbound_tag: inbound.tag,
                protocol_name: "vless",
                listener: quic_inbound,
                shutdown,
                handler: move |engine: Proxy,
                               inbound_tag: String,
                               quic_stream: crate::transport::QuicStream| {
                    let profile = profile.clone();
                    let fallback_config = fallback_config.clone();
                    async move {
                        let result = handle_vless_client(
                            &engine,
                            quic_stream,
                            inbound_tag.as_str(),
                            profile,
                            fallback_config.as_ref(),
                            None,
                        )
                        .await;

                        if let Err(error) = &result {
                            log_listener_connection_error(
                                "vless",
                                inbound_tag.as_str(),
                                &"quic"
                                    .parse()
                                    .unwrap_or(std::net::SocketAddr::from(([0, 0, 0, 0], 0))),
                                error,
                            );
                        }
                    }
                },
            })
            .await;
        }
        crate::protocol_registry::BoundInbound::Tcp(listener) => {
            let ws_config = ws.as_deref().cloned();
            let grpc_config = grpc.as_deref().cloned();
            let h2_config = h2.as_deref().cloned();
            let http_upgrade_config = http_upgrade.as_deref().cloned();
            let split_http_config = split_http.as_deref().cloned();
            let split_http_registry: Option<crate::transport::SplitHttpRegistry> =
                split_http_config
                    .as_ref()
                    .map(|_| crate::transport::SplitHttpRegistry::new());
            let fallback_config = fallback.as_deref().cloned();

            run_tcp_listener_loop(TcpListenerLoopRequest {
                proxy,
                inbound_tag: inbound.tag,
                protocol_name: "vless",
                listener,
                shutdown,
                handler: move |engine: Proxy,
                               inbound_tag: String,
                               stream: TokioSocket,
                               source_addr: Option<std::net::SocketAddr>| {
                    let profile = profile.clone();
                    let tls_acceptor = tls_acceptor.clone();
                    let reality = reality.clone();
                    let ws_config = ws_config.clone();
                    let grpc_config = grpc_config.clone();
                    let h2_config = h2_config.clone();
                    let http_upgrade_config = http_upgrade_config.clone();
                    let split_http_config = split_http_config.clone();
                    let split_http_registry = split_http_registry.clone();
                    let fallback_config = fallback_config.clone();

                    async move {
                        let result: Result<(), EngineError> = async {
                            match (tls_acceptor, reality) {
                                (Some(acceptor), None) => {
                                    let mut raw = stream.into_inner();
                                    let hello =
                                        crate::transport::tls_hello::peek_client_hello(&mut raw)
                                            .await
                                            .ok();

                                    if let Some(hello) = hello {
                                        let socket = TokioSocket::new(raw);
                                        let (socket, replay_head) =
                                            if let Some(fb) = fallback_config.as_ref() {
                                                match vless::fallback_replay_for_alpns(
                                                    fb.alpn.as_deref(),
                                                    hello.alpn.iter().map(|alpn| alpn.as_str()),
                                                    socket,
                                                    hello.consumed,
                                                ) {
                                                    vless::VlessFallbackAlpnDecision::Replay(
                                                        fallback_replay,
                                                    ) => {
                                                        return relay_fallback(
                                                            &engine,
                                                            fallback_replay,
                                                            fb,
                                                        )
                                                        .await;
                                                    }
                                                    vless::VlessFallbackAlpnDecision::Continue {
                                                        stream,
                                                        replay_head,
                                                    } => (stream, replay_head),
                                                }
                                            } else {
                                                (socket, hello.consumed)
                                            };

                                        let sni = hello.sni;
                                        let prefixed =
                                            PrefixedSocket::from_prefix(socket, replay_head);
                                        match acceptor.accept(prefixed).await {
                                            Ok(tls_stream) => {
                                                handle_vless_stream(
                                                    &engine,
                                                    InboundTlsStream::new_generic(tls_stream),
                                                    inbound_tag.as_str(),
                                                    profile.clone(),
                                                    ws_config.as_ref(),
                                                    grpc_config.as_ref(),
                                                    h2_config.as_ref(),
                                                    split_http_config.as_ref(),
                                                    split_http_registry.as_ref(),
                                                    http_upgrade_config.as_ref(),
                                                    fallback_config.as_ref(),
                                                    sni,
                                                )
                                                .await
                                            }
                                            Err(error) => Err(error.into()),
                                        }
                                    } else {
                                        match acceptor.accept(raw).await {
                                            Ok(tls_stream) => {
                                                handle_vless_stream(
                                                    &engine,
                                                    InboundTlsStream::new(tls_stream),
                                                    inbound_tag.as_str(),
                                                    profile.clone(),
                                                    ws_config.as_ref(),
                                                    grpc_config.as_ref(),
                                                    h2_config.as_ref(),
                                                    split_http_config.as_ref(),
                                                    split_http_registry.as_ref(),
                                                    http_upgrade_config.as_ref(),
                                                    fallback_config.as_ref(),
                                                    None,
                                                )
                                                .await
                                            }
                                            Err(error) => Err(error.into()),
                                        }
                                    }
                                }
                                (None, Some(reality)) => {
                                    match upgrade_vless_reality_server(stream, &reality).await {
                                        Ok(reality_stream) => {
                                            handle_vless_stream(
                                                &engine,
                                                reality_stream,
                                                inbound_tag.as_str(),
                                                profile.clone(),
                                                ws_config.as_ref(),
                                                grpc_config.as_ref(),
                                                h2_config.as_ref(),
                                                split_http_config.as_ref(),
                                                split_http_registry.as_ref(),
                                                http_upgrade_config.as_ref(),
                                                fallback_config.as_ref(),
                                                None,
                                            )
                                            .await
                                        }
                                        Err(error) => Err(error.into()),
                                    }
                                }
                                (None, None) => {
                                    handle_vless_stream(
                                        &engine,
                                        stream,
                                        inbound_tag.as_str(),
                                        profile.clone(),
                                        ws_config.as_ref(),
                                        grpc_config.as_ref(),
                                        h2_config.as_ref(),
                                        split_http_config.as_ref(),
                                        split_http_registry.as_ref(),
                                        http_upgrade_config.as_ref(),
                                        fallback_config.as_ref(),
                                        None,
                                    )
                                    .await
                                }
                                (Some(_), Some(_)) => Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidInput,
                                    "vless inbound cannot set both tls and reality",
                                )
                                .into()),
                            }
                        }
                        .await;

                        if let Err(ref error) = result {
                            log_listener_connection_error(
                                "vless",
                                inbound_tag.as_str(),
                                &source_addr,
                                error,
                            );
                        }
                    }
                },
            })
            .await
        }
    }
}
