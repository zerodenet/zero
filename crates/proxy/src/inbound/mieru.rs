//! Mieru inbound encrypted handshake and AEAD-framed relay.

mod udp;

use async_trait::async_trait;
use mieru::{MieruInbound, MieruInboundProfile};
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::logging::log_listener_connection_error;
use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;

type MieruClientStream = mieru::MieruInboundStream<TcpRelayStream>;

struct MieruAcceptedSessionHandler<'a> {
    proxy: &'a Proxy,
    session: Option<Session>,
    client: Option<MieruClientStream>,
    handler: &'a MieruInboundHandler,
    tag: &'a str,
    source_addr: Option<std::net::SocketAddr>,
}

impl mieru::MieruInboundSessionHandler for MieruAcceptedSessionHandler<'_> {
    type Error = EngineError;

    async fn handle_tcp_session(&mut self) -> Result<(), Self::Error> {
        serve_inbound(
            self.proxy,
            self.session
                .take()
                .expect("mieru accepted session is dispatched once"),
            self.client
                .take()
                .expect("mieru accepted client is dispatched once"),
            self.handler,
            self.tag,
            self.source_addr,
        )
        .await
    }

    async fn handle_udp_session(&mut self) -> Result<(), Self::Error> {
        let session = self
            .session
            .take()
            .expect("mieru accepted session is dispatched once");
        self.proxy
            .run_mieru_udp_relay(
                self.client
                    .take()
                    .expect("mieru accepted client is dispatched once"),
                &session,
                self.tag,
            )
            .await
    }
}

#[derive(Debug)]
pub(crate) struct MieruInboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: MieruInboundProfile,
}

// Handler.

#[derive(Clone)]
pub(crate) struct MieruInboundHandler {
    mieru_inbound: MieruInbound,
    profile: MieruInboundProfile,
}

#[async_trait]
impl InboundProtocol for MieruInboundHandler {
    type ClientStream = MieruClientStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let mut metered = crate::transport::MeteredStream::new(stream);
        let accept = self
            .profile
            .accept_request(&self.mieru_inbound, &mut metered)
            .await?;

        let mut client = mieru::MieruInboundStream::new(metered.into_inner(), accept);

        let mut session = client.accept_tunneled_socks5_session().await?;
        session.apply_auth(self.profile.inbound_auth());

        Ok((session, client))
    }

    async fn send_ok(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        Ok(()) // Mieru handshake already confirms success
    }

    async fn send_blocked(&self, _client: &mut Self::ClientStream) -> Result<(), EngineError> {
        // Mieru protocol has no explicit blocked response;
        // the connection close serves as the signal.
        Ok(())
    }

    async fn send_upstream_failure(
        &self,
        _client: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        self.send_blocked(_client).await
    }
}

// Listener.

pub(crate) async fn run_mieru_listener_with_bound(
    proxy: &Proxy,
    request: MieruInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let MieruInboundRequest { inbound, profile } = request;
    let local_addr = listener.local_addr()?;

    let handler = MieruInboundHandler {
        mieru_inbound: MieruInbound,
        profile,
    };

    let mut connections: JoinSet<Result<(), EngineError>> = JoinSet::new();

    info!(
        inbound_tag = %inbound.tag,
        protocol = "mieru",
        listen = %local_addr,
        "inbound listener ready"
    );

    loop {
        select! {
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
                            match handler.accept(stream.into()).await {
                                Ok((session, client)) => {
                                    let dispatch_session = session.clone();
                                    let mut session_handler = MieruAcceptedSessionHandler {
                                        proxy: &engine,
                                        session: Some(session),
                                        client: Some(client),
                                        handler: &handler,
                                        tag: &tag,
                                        source_addr,
                                    };
                                    let _ = mieru::dispatch_inbound_session(
                                        &dispatch_session,
                                        &mut session_handler,
                                    )
                                    .await;
                                }
                                Err(error) => {
                                    log_listener_connection_error(
                                        "mieru", &tag, &remote_addr, &error,
                                    );
                                }
                            }
                            Ok(())
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "mieru: accept error");
                        break;
                    }
                }
            }
            result = connections.join_next(), if !connections.is_empty() => {
                match result {
                    Some(Err(error)) if !error.is_cancelled() => {
                        error!(error = %error, "mieru connection task panicked");
                    }
                    _ => {}
                }
            }
        }
    }

    connections.abort_all();
    while let Some(result) = connections.join_next().await {
        if let Err(error) = result {
            if !error.is_cancelled() {
                error!(error = %error, "mieru shutdown error");
            }
        }
    }

    info!(inbound_tag = %inbound.tag, protocol = "mieru", "listener stopped");
    Ok(())
}
