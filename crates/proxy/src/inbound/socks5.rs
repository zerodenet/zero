use async_trait::async_trait;
use socks5::{Socks5Inbound, Socks5Request};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_engine::EngineError;

use zero_core::Session;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};

mod udp_associate;

// ── New trait-based handler ────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct Socks5InboundHandler {
    socks5_inbound: Socks5Inbound,
    auth: socks5::ConfiguredSocks5PasswordAuth,
}

impl Socks5InboundHandler {
    pub(crate) fn new(
        socks5_inbound: Socks5Inbound,
        auth: socks5::ConfiguredSocks5PasswordAuth,
    ) -> Self {
        Self {
            socks5_inbound,
            auth,
        }
    }

    pub(crate) async fn accept_command(
        &self,
        stream: &mut MeteredStream<TcpRelayStream>,
    ) -> Result<Socks5Request, zero_core::Error> {
        self.socks5_inbound
            .accept_command_with_auth(stream, &self.auth)
            .await
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
        match self.accept_command(&mut metered).await? {
            Socks5Request::Connect(session) => Ok((*session, metered.into_inner())),
            Socks5Request::UdpAssociate(_) => Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "UDP ASSOCIATE - dispatch from listener",
            ))),
        }
    }

    async fn send_ok(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        self.socks5_inbound
            .send_success_response(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self.socks5_inbound.send_blocked_response(client).await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut TcpRelayStream) -> Result<(), EngineError> {
        let _ = self
            .socks5_inbound
            .send_upstream_failure_response(client)
            .await;
        let _ = AsyncWriteExt::shutdown(client).await;
        Ok(())
    }
    // relay uses default
}

pub(crate) struct Socks5InboundRequest {
    pub(crate) inbound: zero_config::InboundConfig,
    pub(crate) auth: socks5::ConfiguredSocks5PasswordAuth,
}

// ── Listener ────────────────────────────────────────────────────────────

pub(crate) async fn run_socks5_listener_with_bound(
    proxy: &Proxy,
    request: Socks5InboundRequest,
    listener: zero_platform_tokio::TokioListener,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let Socks5InboundRequest { inbound, auth } = request;
    let local_addr = listener.local_addr()?;
    let mut connections = JoinSet::new();

    let handler = Socks5InboundHandler::new(socks5::Socks5Inbound, auth);

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
                        let engine = proxy.clone();
                        let tag = inbound.tag.clone();
                        let handler = handler.clone();
                        let source_addr = zero_platform_tokio::remote_ip_to_socket_addr(remote_addr);
                        connections.spawn(async move {
                            let mut metered = MeteredStream::new(TcpRelayStream::from(stream));
                            match handler.accept_command(&mut metered).await {
                                Ok(Socks5Request::Connect(session)) => {
                                    let _ = serve_inbound(
                                        &engine,
                                        *session,
                                        metered.into_inner(),
                                        &handler,
                                        &tag,
                                        source_addr,
                                    )
                                    .await;
                                }
                                Ok(Socks5Request::UdpAssociate(request)) => {
                                    let _ = engine
                                        .handle_socks5_udp_associate(metered, &tag, request)
                                        .await;
                                }
                                Err(err) => {
                                    let engine_err = EngineError::from(err);
                                    log_listener_connection_error(
                                        "socks5",
                                        &tag,
                                        &source_addr,
                                        &engine_err,
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

    info!(
        inbound_tag = %inbound.tag,
        protocol = "socks5",
        listen = %local_addr,
        "inbound listener stopped"
    );
    Ok(())
}
