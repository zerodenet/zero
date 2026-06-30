//! Trojan inbound: TLS accept, protocol auth, route, TCP/UDP relay.

mod udp;

use std::io;

use async_trait::async_trait;
use tokio::select;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, info, warn};
use trojan::{TrojanInbound, TrojanInboundProfile};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::Proxy;
use crate::transport::{AsyncSocketStream, TcpRelayStream};

struct TrojanAcceptedSessionHandler<'a> {
    proxy: &'a Proxy,
    session: Option<Session>,
    client: Option<TcpRelayStream>,
    handler: &'a TrojanInboundHandler,
    tag: &'a str,
    source_addr: Option<std::net::SocketAddr>,
}

impl trojan::TrojanInboundSessionHandler for TrojanAcceptedSessionHandler<'_> {
    type Error = EngineError;

    async fn handle_tcp_session(&mut self) -> Result<(), Self::Error> {
        serve_inbound(
            self.proxy,
            self.session
                .take()
                .expect("trojan accepted session is dispatched once"),
            self.client
                .take()
                .expect("trojan accepted client is dispatched once"),
            self.handler,
            self.tag,
            self.source_addr,
        )
        .await
    }

    async fn handle_udp_session(&mut self) -> Result<(), Self::Error> {
        self.proxy
            .run_trojan_udp_relay(
                self.client
                    .take()
                    .expect("trojan accepted client is dispatched once"),
                self.session
                    .take()
                    .expect("trojan accepted session is dispatched once"),
                self.tag,
            )
            .await
    }
}

pub(crate) struct TrojanInboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: TrojanInboundProfile,
    pub(crate) tls_acceptor: crate::transport::TlsAcceptor,
}

// Trait-based handler.

#[derive(Clone)]
pub(crate) struct TrojanInboundHandler {
    trojan_inbound: TrojanInbound,
    profile: TrojanInboundProfile,
    tls_acceptor: crate::transport::TlsAcceptor,
}

#[async_trait]
impl InboundProtocol for TrojanInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        // TLS accept
        let tls = self
            .tls_acceptor
            .accept(stream)
            .await
            .map_err(|e| EngineError::Io(io::Error::other(e)))?;
        // Trojan protocol auth
        let mut sock = AsyncSocketStream::new(tls);
        let accept = self.profile.accept(self.trojan_inbound, &mut sock).await?;
        let mut session: Session = accept.session;
        session.apply_auth(self.profile.inbound_auth());
        Ok((session, TcpRelayStream::new(sock.into_inner())))
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        // Trojan has no success response
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        // Trojan has no blocked response; just close.
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
    // relay uses default
}

// Listener.

pub(crate) async fn run_trojan_listener_with_bound(
    proxy: &Proxy,
    request: TrojanInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let TrojanInboundRequest {
        inbound,
        profile,
        tls_acceptor,
    } = request;
    let tag = inbound.tag.clone();

    let handler = TrojanInboundHandler {
        trojan_inbound: TrojanInbound,
        profile,
        tls_acceptor,
    };

    info!(inbound_tag = %tag, protocol = "trojan", listen = %listener.local_addr()?, "started");

    let mut conns = JoinSet::new();
    loop {
        select! {
            _ = shutdown.changed() => { if *shutdown.borrow() { break; } }
            r = listener.accept() => {
                let (s, peer) = match r { Ok(v) => v, Err(e) => { error!(%e, "accept"); continue; } };
                let p = proxy.clone();
                let t = tag.clone();
                let h = handler.clone();
                let source_addr = zero_platform_tokio::remote_ip_to_socket_addr(peer);
                conns.spawn(async move {
                    match h.accept(s.into()).await {
                        Ok((session, client)) => {
                            let dispatch_session = session.clone();
                            let mut session_handler = TrojanAcceptedSessionHandler {
                                proxy: &p,
                                session: Some(session),
                                client: Some(client),
                                handler: &h,
                                tag: &t,
                                source_addr,
                            };
                            let result = trojan::dispatch_inbound_session(
                                &dispatch_session,
                                &mut session_handler,
                            )
                            .await;
                            if let Err(e) = result {
                                if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                                    io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                                { warn!(?source_addr, %e, "trojan failed"); }
                            }
                        }
                        Err(e) => {
                            if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                                io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                            { warn!(?source_addr, %e, "trojan auth failed"); }
                        }
                    }
                });
            }
            r = conns.join_next(), if !conns.is_empty() => {
                if let Some(Err(e)) = r { if !e.is_cancelled() { error!(%e, "task panicked"); } }
            }
        }
    }
    conns.abort_all();
    info!(inbound_tag = %tag, "stopped");
    Ok(())
}
