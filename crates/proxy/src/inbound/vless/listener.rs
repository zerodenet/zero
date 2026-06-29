use std::sync::Arc;

use crate::logging::log_listener_connection_error;
use crate::runtime::Proxy;
use crate::transport::{build_tls_acceptor, InboundTlsStream, PrefixedSocket};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_engine::EngineError;
use zero_platform_tokio::TokioSocket;

use super::model::VlessInboundRequest;
use super::session::{VlessStreamRequest, VlessStreamTransport};
use super::upgrade_vless_reality_server;

pub(crate) async fn run_vless_listener_with_bound(
    proxy: &Proxy,
    request: VlessInboundRequest,
    bound: crate::protocol_registry::BoundInbound,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let VlessInboundRequest {
        inbound,
        users,
        reality,
    } = request;
    let listen_addr = format!("{}:{}", inbound.listen.address, inbound.listen.port);

    match bound {
        crate::protocol_registry::BoundInbound::Quic(quic_inbound) => {
            info!(
                inbound_tag = %inbound.tag,
                protocol = "vless",
                listen = %listen_addr,
                transport = "quic",
                "inbound listener ready"
            );
            let mut connections = JoinSet::new();
            let fallback_config = inbound.protocol.vless_fallback().cloned();
            return proxy
                .run_vless_quic_accept_loop(
                    &inbound,
                    &quic_inbound,
                    &mut shutdown,
                    &mut connections,
                    Arc::clone(&users),
                    fallback_config,
                )
                .await;
        }
        crate::protocol_registry::BoundInbound::Tcp(listener) => {
            let local_addr = listener.local_addr()?;
            let tls_acceptor = inbound
                .protocol
                .vless_tls()
                .map(|tls| build_tls_acceptor(tls, proxy.config.source_dir()))
                .transpose()?;
            let ws_config = inbound.protocol.vless_ws().cloned();
            let grpc_config = inbound.protocol.vless_grpc().cloned();
            let h2_config = inbound.protocol.vless_h2().cloned();
            let http_upgrade_config = inbound.protocol.vless_http_upgrade().cloned();
            let split_http_config = inbound.protocol.vless_split_http().cloned();
            let split_http_registry: Option<crate::transport::SplitHttpRegistry> =
                split_http_config
                    .as_ref()
                    .map(|_| crate::transport::SplitHttpRegistry::new());
            let fallback_config = inbound.protocol.vless_fallback().cloned();
            let vless_users = Arc::clone(&users);
            let mut connections = JoinSet::new();

            info!(
                inbound_tag = %inbound.tag,
                protocol = "vless",
                listen = %local_addr,
                tls = tls_acceptor.is_some(),
                reality = reality.is_some(),
                ws = ws_config.is_some(),
                grpc = grpc_config.is_some(),
                http_upgrade = http_upgrade_config.is_some(),
                fallback = fallback_config.is_some(),
                "inbound listener ready"
            );

            loop {
                tokio::select! {
                    changed = shutdown.changed() => {
                        match changed {
                            Ok(()) if *shutdown.borrow() => break,
                            Ok(()) => {}
                            Err(_) => break,
                        }
                    }
                    accept_result = listener.accept() => {
                        let (stream, remote_addr) = accept_result?;
                        let engine = proxy.clone();
                        let inbound_tag = inbound.tag.clone();
                        let vless_users = Arc::clone(&vless_users);
                        let tls_acceptor = tls_acceptor.clone();
                        let reality = reality.clone();
                        let ws_config = ws_config.clone();
                        let grpc_config = grpc_config.clone();
                        let h2_config = h2_config.clone();
                        let http_upgrade_config = http_upgrade_config.clone();
                        let split_http_config = split_http_config.clone();
                        let split_http_registry = split_http_registry.clone();
                        let fallback_config = fallback_config.clone();

                        connections.spawn(async move {
                            let transport = VlessStreamTransport {
                                ws_config: ws_config.as_ref(),
                                grpc_config: grpc_config.as_ref(),
                                h2_config: h2_config.as_ref(),
                                split_http_config: split_http_config.as_ref(),
                                split_http_registry: split_http_registry.as_ref(),
                                http_upgrade_config: http_upgrade_config.as_ref(),
                            };

                            let result = match (tls_acceptor, reality) {
                                (Some(acceptor), None) => {
                                    // Always peek ClientHello to extract SNI for routing.
                                    // Also used for ALPN-based fallback when configured.
                                    let mut raw = stream.into_inner();
                                    let hello = crate::transport::tls_hello::peek_client_hello(
                                        &mut raw,
                                    ).await.ok();

                                    if let Some(hello) = hello {
                                        // Check ALPN fallback match
                                        let alpn_match = fallback_config.as_ref()
                                            .and_then(|fb| fb.alpn.as_ref().zip(Some(fb)))
                                            .and_then(|(expected, fb)| {
                                                hello.alpn.iter()
                                                    .find(|a| *a == expected)
                                                    .map(|_| fb)
                                            });

                                        if let Some(fb) = alpn_match {
                                            let mut upstream = engine.protocols.direct_connector()
                                                .connect_host(&fb.server, fb.port, &engine.resolver)
                                                .await?;
                                            tokio::io::AsyncWriteExt::write_all(
                                                &mut upstream, &hello.consumed,
                                            ).await?;
                                            return engine.relay_fallback_no_tls(
                                                TokioSocket::new(raw), upstream,
                                            ).await;
                                        }

                                        // Continue with TLS accept, replay bytes.
                                        // Pass SNI to the protocol handler for routing.
                                        let sni = hello.sni;
                                        let prefixed = PrefixedSocket::from_prefix(
                                            TokioSocket::new(raw), hello.consumed,
                                        );
                                        match acceptor.accept(prefixed).await {
                                            Ok(tls_stream) => engine
                                                .handle_vless_stream(VlessStreamRequest {
                                                    stream: InboundTlsStream::new_generic(
                                                        tls_stream,
                                                    ),
                                                    inbound_tag: inbound_tag.as_str(),
                                                    users: &vless_users,
                                                    transport,
                                                    fallback: fallback_config.as_ref(),
                                                    sni,
                                                })
                                                .await,
                                            Err(error) => Err(error.into()),
                                        }
                                    } else {
                                        // Not valid TLS; direct TLS accept without peek.
                                        match acceptor.accept(raw).await {
                                            Ok(tls_stream) => engine
                                                .handle_vless_stream(VlessStreamRequest {
                                                    stream: InboundTlsStream::new(tls_stream),
                                                    inbound_tag: inbound_tag.as_str(),
                                                    users: &vless_users,
                                                    transport,
                                                    fallback: fallback_config.as_ref(),
                                                    sni: None,
                                                })
                                                .await,
                                            Err(error) => Err(error.into()),
                                        }
                                    }
                                }
                                (None, Some(reality)) => {
                                    match upgrade_vless_reality_server(stream, &reality).await {
                                        Ok(reality_stream) => {
                                            engine
                                                .handle_vless_stream(VlessStreamRequest {
                                                    stream: reality_stream,
                                                    inbound_tag: inbound_tag.as_str(),
                                                    users: &vless_users,
                                                    transport,
                                                    fallback: fallback_config.as_ref(),
                                                    sni: None,
                                                })
                                                .await
                                        }
                                        Err(error) => Err(error.into()),
                                    }
                                }
                                (None, None) => {
                                    engine
                                        .handle_vless_stream(VlessStreamRequest {
                                            stream,
                                            inbound_tag: inbound_tag.as_str(),
                                            users: &vless_users,
                                            transport,
                                            fallback: fallback_config.as_ref(),
                                            sni: None,
                                        })
                                        .await
                                }
                                (Some(_), Some(_)) => Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidInput,
                                    "vless inbound cannot set both tls and reality",
                                )
                                .into()),
                            };

                            if let Err(ref error) = result {
                                log_listener_connection_error(
                                    "vless",
                                    inbound_tag.as_str(),
                                    &remote_addr,
                                    error,
                                );
                            }
                            result
                        });
                    }
                    result = connections.join_next(), if !connections.is_empty() => {
                        if let Some(Err(error)) = result {
                            if !error.is_cancelled() {
                                error!(error = %error, "vless connection task panicked");
                            }
                        }
                    }
                }
            }

            connections.abort_all();
            while let Some(result) = connections.join_next().await {
                if let Err(error) = result {
                    if !error.is_cancelled() {
                        error!(error = %error, "vless connection task panicked during shutdown");
                    }
                }
            }

            info!(
                inbound_tag = %inbound.tag,
                protocol = "vless",
                listen = %local_addr,
                "inbound listener stopped"
            );

            Ok(())
        }
    }
}
