use std::sync::Arc;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::serve_inbound;
use crate::runtime::Proxy;
use crate::transport::{accept_ws, ClientStream, MeteredStream, TcpRelayStream};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_config::VlessUserConfig;
use zero_engine::EngineError;

use super::*;

impl Proxy {
    pub(crate) async fn run_vless_quic_accept_loop(
        &self,
        inbound: &zero_config::InboundConfig,
        quic_inbound: &crate::transport::QuicInbound,
        shutdown: &mut watch::Receiver<bool>,
        connections: &mut JoinSet<Result<(), EngineError>>,
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
                            let vless_users: Arc<[zero_config::VlessUserConfig]> =
                                inbound.protocol.vless_users().into();
                            let fallback_config = fallback_config.clone();

                            connections.spawn(async move {
                                let result = engine
                                    .handle_vless_client(
                                        quic_stream,
                                        inbound_tag.as_str(),
                                        &vless_users, fallback_config.as_ref(),
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
        stream: S,
        inbound_tag: &str,
        users: &[VlessUserConfig],
        ws_config: Option<&zero_config::WebSocketConfig>,
        grpc_config: Option<&zero_config::GrpcConfig>,
        h2_config: Option<&zero_config::H2Config>,
        split_http_config: Option<&zero_config::SplitHttpConfig>,
        split_http_registry: Option<&crate::transport::SplitHttpRegistry>,
        http_upgrade_config: Option<&zero_config::HttpUpgradeConfig>,
        fallback: Option<&zero_config::FallbackConfig>,
        sni: Option<String>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream + 'static,
    {
        if let Some(cfg) = split_http_config {
            // stream-one / auto: a single bidirectional connection. The server
            // reads the client's POST, responds on the same socket, and the
            // same stream carries upload + download. No registry needed.
            if zero_transport::split_http::XhttpMode::parse(&cfg.mode).is_single_connection() {
                let stream_one = crate::transport::accept_xhttp_stream_one(stream, cfg).await?;
                return self
                    .handle_vless_client(stream_one, inbound_tag, users, fallback, sni)
                    .await;
            }
        }
        if let (Some(cfg), Some(reg)) = (split_http_config, split_http_registry) {
            match crate::transport::accept_split_http(stream, cfg, reg).await? {
                Some(split_stream) => {
                    return self
                        .handle_vless_client(split_stream, inbound_tag, users, fallback, sni)
                        .await;
                }
                None => return Ok(()), // consumed by partner connection
            }
        }
        if let Some(cfg) = http_upgrade_config {
            let upg_stream = crate::transport::accept_http_upgrade(stream, cfg).await?;
            return self
                .handle_vless_client(upg_stream, inbound_tag, users, fallback, sni)
                .await;
        }
        match (ws_config, grpc_config, h2_config) {
            (Some(ws), None, None) => {
                let ws_stream = accept_ws(stream, &ws.path).await?;
                self.handle_vless_client(ws_stream, inbound_tag, users, fallback, sni)
                    .await
            }
            (None, Some(grpc), None) => {
                let engine = self.clone();
                let tag = inbound_tag.to_owned();
                let service_names = grpc.service_names.clone();
                let users_arc: Arc<[VlessUserConfig]> = users.into();
                let fb_clone = fallback.cloned();
                return crate::transport::serve_grpc(stream, &service_names, move |grpc_stream| {
                    let engine = engine.clone();
                    let tag = tag.clone();
                    let users = Arc::clone(&users_arc);
                    let fb = fb_clone.clone();
                    async move {
                        engine
                            .handle_vless_client(grpc_stream, &tag, &users, fb.as_ref(), None)
                            .await
                    }
                })
                .await;
            }
            (None, None, Some(h2)) => {
                let h2_stream = crate::transport::accept_h2(stream, h2).await?;
                self.handle_vless_client(h2_stream, inbound_tag, users, fallback, sni)
                    .await
            }
            (None, None, None) => {
                self.handle_vless_client(stream, inbound_tag, users, fallback, sni)
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
        users: &[VlessUserConfig],
        fallback: Option<&zero_config::FallbackConfig>,
        sni: Option<String>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream + 'static,
    {
        let mut metered = MeteredStream::new(RecordingStream::new(client));
        let auth = ConfiguredVlessUsers { users };
        let result = self
            .protocols
            .vless_inbound
            .accept_tcp_with_auth_and_id(&mut metered, &auth)
            .await;

        let (mut session, uuid) = match result {
            Ok(x) => x,
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

        if vless::VlessInbound::is_mux_session(&session) {
            self.handle_vless_mux_session(client, inbound_tag, uuid, &auth)
                .await
        } else if session.network == zero_core::Network::Udp {
            self.handle_vless_udp_session(client, inbound_tag, session, &auth)
                .await
        } else {
            let handler = VlessInboundHandler {
                vless_inbound: self.protocols.vless_inbound,
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
