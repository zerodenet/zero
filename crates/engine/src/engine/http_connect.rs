use std::time::Instant;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_core::Error as CoreError;
use zero_platform_tokio::TokioSocket;
use zero_protocol_http_connect::HttpConnectResponse;
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
    pub(crate) async fn run_http_connect_listener(
        &self,
        inbound: zero_config::InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let listener = bind_listener(&inbound).await?;
        let local_addr = listener.local_addr()?;
        let mut connections = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "http-connect",
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

                    connections.spawn(async move {
                        if let Err(error) = engine
                            .handle_http_connect_connection(stream, inbound_tag.as_str())
                            .await
                        {
                            log_listener_connection_error(
                                "http-connect",
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
                            error!(error = %error, "http-connect connection task panicked");
                        }
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "http-connect connection task panicked during shutdown");
                }
            }
        }

        info!(
            inbound_tag = %inbound.tag,
            protocol = "http-connect",
            listen = %local_addr,
            "inbound listener stopped"
        );

        Ok(())
    }

    pub(crate) async fn handle_http_connect_connection(
        &self,
        client: TokioSocket,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        self.handle_http_connect_client(client, inbound_tag).await
    }

    pub(crate) async fn handle_http_connect_client<S>(
        &self,
        client: S,
        inbound_tag: &str,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let mut client = MeteredStream::new(client);
        let mut session = match self
            .protocols
            .http_connect_inbound
            .accept_request(&mut client)
            .await
        {
            Ok(session) => session,
            Err(CoreError::Unsupported(_)) => {
                self.reply_and_close_http(&mut client, HttpConnectResponse::MethodNotAllowed)
                    .await;
                return Ok(());
            }
            Err(CoreError::Protocol(_)) => {
                self.reply_and_close_http(&mut client, HttpConnectResponse::BadRequest)
                    .await;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
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
                    .http_connect_inbound
                    .send_response(&mut client, HttpConnectResponse::ConnectionEstablished)
                    .await?;
                let session_id = session.id;
                self.record_session_inbound_traffic(session_id, client.drain_traffic());
                let client = client.into_tokio_socket();
                let upload_engine = self.clone();
                let download_engine = self.clone();

                match relay_bidirectional_metered(
                    client,
                    upstream,
                    move |bytes| upload_engine.record_session_upload(session_id, bytes),
                    move |bytes| download_engine.record_session_download(session_id, bytes),
                )
                .await
                {
                    Ok(_) => {
                        if let Some(record) = session_handle.finish(SessionOutcome::DirectRelayed) {
                            log_session_finished(&record, None);
                        }
                    }
                    Err(error) => {
                        let record = session_handle.finish(SessionOutcome::Failed);
                        log_session_failed(
                            &session,
                            record.as_ref(),
                            "relay",
                            started_at.elapsed(),
                            &error,
                            None,
                        );
                        return Err(error.into());
                    }
                }

                Ok(())
            }
            Ok(EstablishedTcpOutbound::Block { tag }) => {
                session.outbound_tag = Some(tag);
                self.set_session_outbound(&session);
                self.reply_and_close_http(&mut client, HttpConnectResponse::Forbidden)
                    .await;
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
            }) => {
                session.outbound_tag = Some(tag);
                self.set_session_outbound(&session);
                self.protocols
                    .http_connect_inbound
                    .send_response(&mut client, HttpConnectResponse::ConnectionEstablished)
                    .await?;
                let session_id = session.id;
                self.record_session_inbound_traffic(session_id, client.drain_traffic());
                let client = client.into_tokio_socket();
                let upload_engine = self.clone();
                let download_engine = self.clone();

                match relay_bidirectional_metered(
                    client,
                    upstream,
                    move |bytes| upload_engine.record_session_upload(session_id, bytes),
                    move |bytes| download_engine.record_session_download(session_id, bytes),
                )
                .await
                {
                    Ok(_) => {
                        if let Some(record) = session_handle.finish(SessionOutcome::ChainedRelayed)
                        {
                            log_session_finished(&record, Some((&server, port)));
                        }
                    }
                    Err(error) => {
                        let record = session_handle.finish(SessionOutcome::Failed);
                        log_session_failed(
                            &session,
                            record.as_ref(),
                            "relay",
                            started_at.elapsed(),
                            &error,
                            Some((&server, port)),
                        );
                        return Err(error.into());
                    }
                }

                Ok(())
            }
            Err(failure) => {
                self.reply_and_close_http(&mut client, HttpConnectResponse::BadGateway)
                    .await;
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

    pub(crate) async fn reply_and_close_http(
        &self,
        client: &mut impl AsyncSocket<Error = std::io::Error>,
        response: HttpConnectResponse,
    ) {
        if let Err(error) = self
            .protocols
            .http_connect_inbound
            .send_response(client, response)
            .await
        {
            error!(error = %error, "failed to write http-connect response");
        }

        if let Err(error) = client.shutdown().await {
            error!(error = %error, "failed to shutdown client socket");
        }
    }
}
