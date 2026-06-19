use async_trait::async_trait;
use socks5::Socks5PasswordAuth;
use socks5::{Socks5Inbound, Socks5Reply, Socks5Request};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_config::Socks5UserConfig;
use zero_engine::EngineError;

use zero_core::Session;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

// ── New trait-based handler ────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct Socks5InboundHandler {
    socks5_inbound: Socks5Inbound,
    pub(crate) users: Vec<Socks5UserConfig>,
}

impl Socks5InboundHandler {
    pub(crate) fn new(socks5_inbound: Socks5Inbound, users: Vec<Socks5UserConfig>) -> Self {
        Self {
            socks5_inbound,
            users,
        }
    }

    pub(crate) fn socks5_inbound(&self) -> Socks5Inbound {
        self.socks5_inbound
    }
}

#[async_trait]
impl InboundProtocol for Socks5InboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let mut metered = MeteredStream::new(stream);
        let auth = ConfiguredSocks5PasswordAuth { users: &self.users };
        match self
            .socks5_inbound
            .accept_command_with_auth(&mut metered, &auth)
            .await?
        {
            Socks5Request::Connect(session) => Ok((session, metered.into_inner())),
            Socks5Request::UdpAssociate(_) => Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "UDP ASSOCIATE — dispatch from listener",
            ))),
        }
    }

    async fn send_ok(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        self.socks5_inbound
            .send_response(client, Socks5Reply::Succeeded)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self
            .socks5_inbound
            .send_response(client, Socks5Reply::ConnectionNotAllowed)
            .await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self
            .socks5_inbound
            .send_response(client, Socks5Reply::HostUnreachable)
            .await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }
    // relay uses default
}

// ── Listener ────────────────────────────────────────────────────────────

impl Proxy {
    pub(crate) async fn run_socks5_listener_with_bound(
        &self,
        inbound: zero_config::InboundConfig,
        listener: zero_platform_tokio::TokioListener,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), EngineError> {
        let local_addr = listener.local_addr()?;
        let mut connections = JoinSet::new();

        let users = inbound.protocol.socks5_users().to_vec();
        let handler = Socks5InboundHandler {
            socks5_inbound: self.protocols.socks5_inbound_protocol(),
            users,
        };

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
                    match accept_result {
                        Ok((stream, remote_addr)) => {
                            let engine = self.clone();
                            let tag = inbound.tag.clone();
                            let handler = handler.clone();
                            let source_addr = remote_addr
                                .map(|ip| match ip {
                                    zero_traits::IpAddress::V4(octets) => {
                                        std::net::SocketAddr::new(
                                            std::net::IpAddr::V4(std::net::Ipv4Addr::from(octets)),
                                            0,
                                        )
                                    }
                                    zero_traits::IpAddress::V6(octets) => {
                                        std::net::SocketAddr::new(
                                            std::net::IpAddr::V6(std::net::Ipv6Addr::from(octets)),
                                            0,
                                        )
                                    }
                                });
                            connections.spawn(async move {
                                let metered = MeteredStream::new(
                                    TcpRelayStream::from(stream),
                                );
                                let mut metered = metered;
                                let auth = ConfiguredSocks5PasswordAuth {
                                    users: &handler.users,
                                };
                                match handler.socks5_inbound
                                    .accept_command_with_auth(&mut metered, &auth).await
                                {
                                    Ok(Socks5Request::Connect(session)) => {
                                        let _ = serve_inbound(
                                            &engine, session, metered.into_inner(),
                                            &handler, &tag, source_addr,
                                        ).await;
                                    }
                                    Ok(Socks5Request::UdpAssociate(request)) => {
                                        let _ = engine
                                            .handle_socks5_udp_associate(
                                                metered, &tag, request,
                                            ).await;
                                    }
                                    Err(err) => {
                                        let engine_err = EngineError::from(err);
                                        log_listener_connection_error(
                                            "socks5", &tag, &source_addr, &engine_err,
                                        );
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            error!(error = %e, "socks5: accept error");
                        }
                    }
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

        info!(inbound_tag = %inbound.tag, protocol = "socks5", listen = %local_addr, "inbound listener stopped");
        Ok(())
    }
}

// ── Auth ────────────────────────────────────────────────────────────────

pub(crate) struct ConfiguredSocks5PasswordAuth<'a> {
    pub(crate) users: &'a [Socks5UserConfig],
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

    fn principal_key_for(&self, username: &str) -> Option<String> {
        self.users
            .iter()
            .find(|user| user.username == username)
            .and_then(|user| user.principal_key.clone())
    }

    fn rate_limit_for(&self, username: &str) -> (Option<u64>, Option<u64>) {
        self.users
            .iter()
            .find(|user| user.username == username)
            .map(|user| (user.up_bps, user.down_bps))
            .unwrap_or((None, None))
    }
}
