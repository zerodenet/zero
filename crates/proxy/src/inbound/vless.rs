use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_config::{InboundRealityConfig, VlessUserConfig};
use zero_protocol_vless::RealityServerOptions;
use zero_protocol_vless::{VlessUser, VlessUserStore};
use zero_traits::AsyncSocket;

use super::super::logging::log_listener_connection_error;
use super::super::runtime::{bind_listener, Proxy};
use super::super::transport::ClientStream;
use super::super::transport::MeteredStream;
use super::super::transport::TcpInboundProtocol;
use super::super::transport::{accept_ws, build_tls_acceptor, InboundTlsStream};
use zero_engine::EngineError;

impl Proxy {
    pub(crate) async fn run_vless_listener(
        &self,
        inbound: zero_config::InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let listener = bind_listener(&inbound).await?;
        let local_addr = listener.local_addr()?;
        let tls_acceptor = inbound
            .protocol
            .vless_tls()
            .map(|tls| build_tls_acceptor(tls, self.config.source_dir()))
            .transpose()?;
        let reality_config = inbound.protocol.vless_reality().cloned();
        let ws_config = inbound.protocol.vless_ws().cloned();
        let mut connections = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "vless",
            listen = %local_addr,
            tls = tls_acceptor.is_some(),
            reality = reality_config.is_some(),
            ws = ws_config.is_some(),
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
                    let engine = self.clone();
                    let inbound_tag = inbound.tag.clone();
                    let vless_users = inbound.protocol.vless_users().to_vec();
                    let tls_acceptor = tls_acceptor.clone();
                    let reality_config = reality_config.clone();
                    let ws_config = ws_config.clone();

                    connections.spawn(async move {
                        let result = match (tls_acceptor, reality_config) {
                            (Some(acceptor), None) => {
                                match acceptor.accept(stream.into_inner()).await {
                                    Ok(tls_stream) => {
                                        engine
                                            .handle_vless_stream(
                                                InboundTlsStream::new(tls_stream),
                                                inbound_tag.as_str(),
                                                &vless_users,
                                                ws_config.as_ref(),
                                            )
                                            .await
                                    }
                                    Err(error) => Err(error.into()),
                                }
                            }
                            (None, Some(reality)) => {
                                match upgrade_vless_reality_server(stream, &reality).await {
                                    Ok(reality_stream) => {
                                        engine
                                            .handle_vless_stream(
                                                reality_stream,
                                                inbound_tag.as_str(),
                                                &vless_users,
                                                ws_config.as_ref(),
                                            )
                                            .await
                                    }
                                    Err(error) => Err(error.into()),
                                }
                            }
                            (None, None) => {
                                engine
                                    .handle_vless_stream(
                                        stream,
                                        inbound_tag.as_str(),
                                        &vless_users,
                                        ws_config.as_ref(),
                                    )
                                    .await
                            }
                            (Some(_), Some(_)) => Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "vless inbound cannot set both tls and reality",
                            )
                            .into()),
                        };

                        if let Err(error) = result {
                            log_listener_connection_error(
                                "vless",
                                inbound_tag.as_str(),
                                &remote_addr,
                                &error,
                            );
                        }
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

    async fn handle_vless_stream<S>(
        &self,
        stream: S,
        inbound_tag: &str,
        users: &[VlessUserConfig],
        ws_config: Option<&zero_config::WebSocketConfig>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream + 'static,
    {
        match ws_config {
            Some(ws) => {
                let ws_stream = accept_ws(stream, &ws.path).await?;
                self.handle_vless_client(ws_stream, inbound_tag, users)
                    .await
            }
            None => self.handle_vless_client(stream, inbound_tag, users).await,
        }
    }

    pub(crate) async fn handle_vless_client<S>(
        &self,
        client: S,
        inbound_tag: &str,
        users: &[VlessUserConfig],
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let mut client = MeteredStream::new(client);
        let auth = ConfiguredVlessUsers { users };
        let session = self
            .protocols
            .vless_inbound
            .accept_tcp_with_auth(&mut client, &auth)
            .await?;

        self.handle_tcp_session(client, inbound_tag, session, TcpInboundProtocol::Vless)
            .await
    }

    pub(crate) async fn close_vless_client(
        &self,
        client: &mut impl AsyncSocket<Error = std::io::Error>,
    ) {
        if let Err(error) = client.shutdown().await {
            error!(error = %error, "failed to shutdown client socket");
        }
    }
}

async fn upgrade_vless_reality_server<S>(
    stream: S,
    reality: &InboundRealityConfig,
) -> std::io::Result<zero_protocol_vless::RealityTlsStream<S>>
where
    S: ClientStream + 'static,
{
    let server_name = reality.server_name.as_deref().unwrap_or("localhost");
    zero_protocol_vless::upgrade_reality_server(
        stream,
        RealityServerOptions {
            private_key: &reality.private_key,
            short_ids: &reality.short_ids,
            server_name,
            cipher_suites: &reality.cipher_suites,
        },
    )
    .await
}

struct ConfiguredVlessUsers<'a> {
    users: &'a [VlessUserConfig],
}

impl VlessUserStore for ConfiguredVlessUsers<'_> {
    fn find_user(&self, id: &[u8; 16]) -> Option<VlessUser> {
        self.users.iter().find_map(|user| {
            let configured_id = zero_protocol_vless::parse_uuid(&user.id).ok()?;
            if &configured_id == id {
                Some(VlessUser {
                    credential_id: user.credential_id.clone(),
                    principal_key: user.principal_key.clone(),
                })
            } else {
                None
            }
        })
    }
}
