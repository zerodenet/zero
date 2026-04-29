use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_core::Error as CoreError;
use zero_platform_tokio::TokioSocket;
use zero_protocol_http_connect::HttpConnectResponse;
use zero_traits::AsyncSocket;

use super::super::logging::log_listener_connection_error;
use super::super::runtime::{bind_listener, Proxy};
use super::super::transport::ClientStream;
use super::super::transport::MeteredStream;
use super::super::transport::TcpInboundProtocol;
use zero_engine::EngineError;

impl Proxy {
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
        let session = match self
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
        self.handle_tcp_session(
            client,
            inbound_tag,
            session,
            TcpInboundProtocol::HttpConnect,
        )
        .await
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
