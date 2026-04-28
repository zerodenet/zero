use std::time::Instant;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_config::VlessUserConfig;
use zero_platform_tokio::TokioSocket;
use zero_protocol_vless::{VlessUser, VlessUserStore};
use zero_traits::AsyncSocket;

use super::error::EngineError;
use super::logging::{
    log_listener_connection_error, log_session_accepted, log_session_failed, log_session_finished,
};
use super::metered::MeteredStream;
use super::runtime::{bind_listener, Engine};
use super::stats::SessionOutcome;
use super::stream::ClientStream;
use super::tcp_outbound::EstablishedTcpOutbound;
use super::tcp_relay::relay_bidirectional_metered;

impl Engine {
    pub(crate) async fn run_vless_listener(
        &self,
        inbound: zero_config::InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let listener = bind_listener(&inbound).await?;
        let local_addr = listener.local_addr()?;
        let mut connections = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "vless",
            listen = %local_addr,
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

                    connections.spawn(async move {
                        if let Err(error) = engine
                            .handle_vless_connection(stream, inbound_tag.as_str(), &vless_users)
                            .await
                        {
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

    pub(crate) async fn handle_vless_connection(
        &self,
        client: TokioSocket,
        inbound_tag: &str,
        users: &[VlessUserConfig],
    ) -> Result<(), EngineError> {
        self.handle_vless_client(client, inbound_tag, users).await
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
        let mut session = self
            .protocols
            .vless_inbound
            .accept_tcp_with_auth(&mut client, &auth)
            .await?;

        self.prepare_session(&mut session, inbound_tag);
        let mut session_handle = self.track_session(session.id);
        let started_at = Instant::now();
        self.record_session_inbound_traffic(session.id, client.drain_traffic());

        let action = self.route_decision(&session.target);
        let resolved = match self.resolve_outbound(action) {
            Ok(resolved) => resolved,
            Err(error) => {
                let record = session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &session,
                    record.as_ref(),
                    "resolve_outbound",
                    started_at.elapsed(),
                    &error,
                    None,
                );
                return Err(error);
            }
        };
        log_session_accepted(&session, &action, self.config.mode.kind());

        match self.establish_tcp_outbound(&session, resolved).await {
            Ok(EstablishedTcpOutbound::Direct { tag, upstream }) => {
                session.outbound_tag = Some(tag);
                self.set_session_outbound(&session);
                self.protocols
                    .vless_inbound
                    .send_response(&mut client)
                    .await?;
                self.relay_vless_session(VlessRelayContext {
                    client,
                    upstream,
                    session,
                    session_handle,
                    outcome: SessionOutcome::DirectRelayed,
                    started_at,
                    upstream_endpoint: None,
                })
                .await
            }
            Ok(EstablishedTcpOutbound::Block { tag }) => {
                session.outbound_tag = Some(tag);
                self.set_session_outbound(&session);
                self.close_vless_client(&mut client).await;
                self.record_session_inbound_traffic(session.id, client.drain_traffic());
                if let Some(record) = session_handle.finish(SessionOutcome::Blocked) {
                    log_session_finished(&record, None);
                }

                Ok(())
            }
            Ok(EstablishedTcpOutbound::Socks5 {
                tag,
                server,
                port,
                upstream,
            })
            | Ok(EstablishedTcpOutbound::Vless {
                tag,
                server,
                port,
                upstream,
            }) => {
                session.outbound_tag = Some(tag);
                self.set_session_outbound(&session);
                self.protocols
                    .vless_inbound
                    .send_response(&mut client)
                    .await?;
                self.relay_vless_session(VlessRelayContext {
                    client,
                    upstream,
                    session,
                    session_handle,
                    outcome: SessionOutcome::ChainedRelayed,
                    started_at,
                    upstream_endpoint: Some((server, port)),
                })
                .await
            }
            Err(failure) => {
                self.close_vless_client(&mut client).await;
                self.record_session_inbound_traffic(session.id, client.drain_traffic());
                let record = session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &session,
                    record.as_ref(),
                    failure.stage,
                    started_at.elapsed(),
                    &failure.error,
                    failure
                        .upstream_endpoint
                        .as_ref()
                        .map(|(server, port)| (server.as_str(), *port)),
                );
                Err(failure.error)
            }
        }
    }

    async fn relay_vless_session<S>(
        &self,
        mut context: VlessRelayContext<S>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let session_id = context.session.id;
        self.record_session_inbound_traffic(session_id, context.client.drain_traffic());
        let client = context.client.into_tokio_socket();
        let upload_engine = self.clone();
        let download_engine = self.clone();

        match relay_bidirectional_metered(
            client,
            context.upstream,
            move |bytes| upload_engine.record_session_upload(session_id, bytes),
            move |bytes| download_engine.record_session_download(session_id, bytes),
        )
        .await
        {
            Ok(_) => {
                if let Some(record) = context.session_handle.finish(context.outcome) {
                    log_session_finished(
                        &record,
                        context
                            .upstream_endpoint
                            .as_ref()
                            .map(|(server, port)| (server.as_str(), *port)),
                    );
                }
                Ok(())
            }
            Err(error) => {
                let record = context.session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &context.session,
                    record.as_ref(),
                    "relay",
                    context.started_at.elapsed(),
                    &error,
                    context
                        .upstream_endpoint
                        .as_ref()
                        .map(|(server, port)| (server.as_str(), *port)),
                );
                Err(error.into())
            }
        }
    }

    async fn close_vless_client(&self, client: &mut impl AsyncSocket<Error = std::io::Error>) {
        if let Err(error) = client.shutdown().await {
            error!(error = %error, "failed to shutdown client socket");
        }
    }
}

struct ConfiguredVlessUsers<'a> {
    users: &'a [VlessUserConfig],
}

struct VlessRelayContext<S> {
    client: MeteredStream<S>,
    upstream: TokioSocket,
    session: zero_core::Session,
    session_handle: super::session_lifecycle::SessionHandle,
    outcome: SessionOutcome,
    started_at: Instant,
    upstream_endpoint: Option<(String, u16)>,
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
