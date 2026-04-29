use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_config::Socks5UserConfig;
use zero_platform_tokio::TokioSocket;
use zero_protocol_socks5::Socks5PasswordAuth;
use zero_protocol_socks5::{Socks5Reply, Socks5Request};
use zero_traits::AsyncSocket;

use super::super::logging::log_listener_connection_error;
use super::super::runtime::{bind_listener, Proxy};
use super::super::transport::ClientStream;
use super::super::transport::MeteredStream;
use super::super::transport::TcpInboundProtocol;
use zero_engine::EngineError;

impl Proxy {
    pub(crate) async fn run_socks5_listener(
        &self,
        inbound: zero_config::InboundConfig,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let listener = bind_listener(&inbound).await?;
        let local_addr = listener.local_addr()?;
        let mut connections = JoinSet::new();

        info!(
            inbound_tag = %inbound.tag,
            protocol = "socks5",
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
                    let socks5_users = inbound.protocol.socks5_users().to_vec();

                    connections.spawn(async move {
                        if let Err(error) = engine
                            .handle_socks5_connection(
                                stream,
                                inbound_tag.as_str(),
                                &socks5_users,
                            )
                            .await
                        {
                            log_listener_connection_error(
                                "socks5",
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
                            error!(error = %error, "socks5 connection task panicked");
                        }
                    }
                }
            }
        }

        connections.abort_all();
        while let Some(result) = connections.join_next().await {
            if let Err(error) = result {
                if !error.is_cancelled() {
                    error!(error = %error, "socks5 connection task panicked during shutdown");
                }
            }
        }

        info!(
            inbound_tag = %inbound.tag,
            protocol = "socks5",
            listen = %local_addr,
            "inbound listener stopped"
        );

        Ok(())
    }

    pub(crate) async fn handle_socks5_connection(
        &self,
        client: TokioSocket,
        inbound_tag: &str,
        users: &[Socks5UserConfig],
    ) -> Result<(), EngineError> {
        self.handle_socks5_client(client, inbound_tag, users).await
    }

    pub(crate) async fn handle_socks5_client<S>(
        &self,
        client: S,
        inbound_tag: &str,
        users: &[Socks5UserConfig],
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let mut client = MeteredStream::new(client);
        let auth = ConfiguredSocks5PasswordAuth { users };
        match self
            .protocols
            .socks5_inbound
            .accept_command_with_auth(&mut client, &auth)
            .await?
        {
            Socks5Request::Connect(session) => {
                self.handle_tcp_session(client, inbound_tag, session, TcpInboundProtocol::Socks5)
                    .await
            }
            Socks5Request::UdpAssociate(request) => {
                self.handle_socks5_udp_associate(client, inbound_tag, request)
                    .await
            }
        }
    }

    pub(crate) async fn reply_and_close_socks5(
        &self,
        client: &mut impl AsyncSocket<Error = std::io::Error>,
        reply: Socks5Reply,
    ) {
        if let Err(error) = self
            .protocols
            .socks5_inbound
            .send_response(client, reply)
            .await
        {
            error!(error = %error, "failed to write socks5 response");
        }

        if let Err(error) = client.shutdown().await {
            error!(error = %error, "failed to shutdown client socket");
        }
    }
}

struct ConfiguredSocks5PasswordAuth<'a> {
    users: &'a [Socks5UserConfig],
}

impl Socks5PasswordAuth for ConfiguredSocks5PasswordAuth<'_> {
    fn required(&self) -> bool {
        !self.users.is_empty()
    }

    fn verify(&self, username: &str, password: &str) -> bool {
        self.users
            .iter()
            .any(|user| user.username == username && user.password == password)
    }
}
