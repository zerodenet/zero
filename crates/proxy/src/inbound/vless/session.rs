use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::serve_inbound;
use crate::runtime::Proxy;
use crate::transport::{accept_ws, ClientStream, MeteredStream, TcpRelayStream};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_engine::EngineError;

use super::{RecordingStream, VlessInboundHandler};

#[derive(Clone, Copy)]
pub(crate) struct VlessStreamTransport<'a> {
    pub(crate) ws_config: Option<&'a zero_config::WebSocketConfig>,
    pub(crate) grpc_config: Option<&'a zero_config::GrpcConfig>,
    pub(crate) h2_config: Option<&'a zero_config::H2Config>,
    pub(crate) split_http_config: Option<&'a zero_config::SplitHttpConfig>,
    pub(crate) split_http_registry: Option<&'a crate::transport::SplitHttpRegistry>,
    pub(crate) http_upgrade_config: Option<&'a zero_config::HttpUpgradeConfig>,
}

pub(crate) struct VlessStreamRequest<'a, S> {
    pub(crate) stream: S,
    pub(crate) inbound_tag: &'a str,
    pub(crate) profile: vless::VlessInboundProfile,
    pub(crate) transport: VlessStreamTransport<'a>,
    pub(crate) fallback: Option<&'a zero_config::FallbackConfig>,
    pub(crate) sni: Option<String>,
}

impl Proxy {
    pub(crate) async fn run_vless_quic_accept_loop(
        &self,
        inbound: &zero_config::InboundConfig,
        quic_inbound: &crate::transport::QuicInbound,
        shutdown: &mut watch::Receiver<bool>,
        connections: &mut JoinSet<Result<(), EngineError>>,
        profile: vless::VlessInboundProfile,
        fallback_config: Option<zero_config::FallbackConfig>,
    ) -> Result<(), EngineError> {
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    match changed {
                        Ok(()) if *shutdown.borrow() => break,
                        Ok(()) => {}
                        Err(_) => break,
                    }
                }
                accept_result = quic_inbound.accept() => {
                    match accept_result {
                        Ok(quic_stream) => {
                            let engine = self.clone();
                            let inbound_tag = inbound.tag.clone();
                            let profile = profile.clone();
                            let fallback_config = fallback_config.clone();

                            connections.spawn(async move {
                                let result = engine
                                    .handle_vless_client(
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
                                        &"quic".parse().unwrap_or(std::net::SocketAddr::from(([0, 0, 0, 0], 0))),
                                        error,
                                    );
                                }
                                result
                            });
                        }
                        Err(error) => {
                            error!(error = %error, "vless quic accept error");
                            break;
                        }
                    }
                }
                result = connections.join_next(), if !connections.is_empty() => {
                    if let Some(Err(error)) = result {
                        if !error.is_cancelled() {
                            error!(error = %error, "vless quic connection task panicked");
                        }
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "vless quic connection task panicked during shutdown");
                }
            }
        }

        info!(
            inbound_tag = %inbound.tag,
            protocol = "vless",
            transport = "quic",
            "inbound listener stopped"
        );

        Ok(())
    }

    pub(crate) async fn handle_vless_stream<S>(
        &self,
        request: VlessStreamRequest<'_, S>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream + 'static,
    {
        let VlessStreamRequest {
            stream,
            inbound_tag,
            profile,
            transport,
            fallback,
            sni,
        } = request;
        let VlessStreamTransport {
            ws_config,
            grpc_config,
            h2_config,
            split_http_config,
            split_http_registry,
            http_upgrade_config,
        } = transport;

        if let Some(cfg) = split_http_config {
            let Some(reg) = split_http_registry else {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "vless inbound: split-http registry is required",
                )));
            };
            match crate::transport::accept_xhttp_inbound(stream, cfg, reg).await? {
                Some(xhttp_stream) => {
                    return self
                        .handle_vless_client(xhttp_stream, inbound_tag, profile, fallback, sni)
                        .await;
                }
                None => return Ok(()),
            }
        }
        if let Some(cfg) = http_upgrade_config {
            let upg_stream = crate::transport::accept_http_upgrade(stream, cfg).await?;
            return self
                .handle_vless_client(upg_stream, inbound_tag, profile, fallback, sni)
                .await;
        }
        match (ws_config, grpc_config, h2_config) {
            (Some(ws), None, None) => {
                let ws_stream = accept_ws(stream, &ws.path).await?;
                self.handle_vless_client(ws_stream, inbound_tag, profile, fallback, sni)
                    .await
            }
            (None, Some(grpc), None) => {
                let engine = self.clone();
                let tag = inbound_tag.to_owned();
                let service_names = grpc.service_names.clone();
                let profile = profile.clone();
                let fb_clone = fallback.cloned();
                return crate::transport::serve_grpc(stream, &service_names, move |grpc_stream| {
                    let engine = engine.clone();
                    let tag = tag.clone();
                    let profile = profile.clone();
                    let fb = fb_clone.clone();
                    async move {
                        engine
                            .handle_vless_client(grpc_stream, &tag, profile, fb.as_ref(), None)
                            .await
                    }
                })
                .await;
            }
            (None, None, Some(h2)) => {
                let h2_stream = crate::transport::accept_h2(stream, h2).await?;
                self.handle_vless_client(h2_stream, inbound_tag, profile, fallback, sni)
                    .await
            }
            (None, None, None) => {
                self.handle_vless_client(stream, inbound_tag, profile, fallback, sni)
                    .await
            }
            _ => Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "vless inbound: ws, grpc, and h2 are mutually exclusive",
            ))),
        }
    }

    pub(crate) async fn handle_vless_client<S>(
        &self,
        client: S,
        inbound_tag: &str,
        profile: vless::VlessInboundProfile,
        fallback: Option<&zero_config::FallbackConfig>,
        sni: Option<String>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream + 'static,
    {
        let mut metered = MeteredStream::new(RecordingStream::new(client));
        let result = profile
            .accept_tcp_with_auth_context(vless::VlessInbound, &mut metered)
            .await;

        let (mut session, mux_context) = match result {
            Ok(accepted) => accepted.into_parts(),
            Err(auth_error) => {
                if let Some(fb) = fallback {
                    let (inner, head) = metered.into_inner().into_parts();
                    return self.relay_fallback(inner, head, fb).await;
                }
                return Err(EngineError::Core(auth_error));
            }
        };

        let (inner_stream, _head) = metered.into_inner().into_parts();
        let client = MeteredStream::new(inner_stream);

        session.sni = sni;

        let auth = session.auth.clone();

        match vless::classify_inbound_session(&session) {
            vless::VlessInboundSessionKind::Mux => {
                self.handle_vless_mux_session(client, inbound_tag, mux_context, &auth)
                    .await
            }
            vless::VlessInboundSessionKind::Udp => {
                self.handle_vless_udp_session(client, inbound_tag, session, &auth)
                    .await
            }
            vless::VlessInboundSessionKind::Tcp => {
                let handler = VlessInboundHandler {
                    vless_inbound: vless::VlessInbound,
                };
                let source_addr = client.peer_addr().ok();
                serve_inbound(
                    self,
                    session,
                    TcpRelayStream::new(client.into_inner()),
                    &handler,
                    inbound_tag,
                    source_addr,
                )
                .await
            }
        }
    }
}
