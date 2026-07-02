//! Trojan inbound: TLS accept, protocol auth, route, TCP/UDP relay.

mod udp;

use std::io;

use async_trait::async_trait;
use tokio::sync::watch;
use tracing::warn;
use trojan::{TrojanInbound, TrojanInboundAcceptedSessionDispatcher, TrojanInboundProfile};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::listener_loop::{run_tcp_listener_loop, TcpListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::{AsyncSocketStream, TcpRelayStream};

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

struct TrojanAcceptedSessionBridge<'a> {
    proxy: &'a Proxy,
    handler: &'a TrojanInboundHandler,
    inbound_tag: &'a str,
    source_addr: Option<std::net::SocketAddr>,
}

#[async_trait]
impl InboundProtocol for TrojanInboundHandler {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        let _ = stream;
        unreachable!("trojan accept is handled before serve_inbound dispatch")
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

type TrojanAcceptedStream =
    AsyncSocketStream<tokio_rustls::server::TlsStream<zero_platform_tokio::TokioSocket>>;

impl TrojanInboundAcceptedSessionDispatcher<TrojanAcceptedStream>
    for TrojanAcceptedSessionBridge<'_>
{
    type Error = EngineError;

    async fn dispatch_tcp_session(
        &mut self,
        session: Session,
        stream: TrojanAcceptedStream,
    ) -> Result<(), Self::Error> {
        serve_inbound(
            self.proxy,
            session,
            TcpRelayStream::new(stream.into_inner()),
            self.handler,
            self.inbound_tag,
            self.source_addr,
        )
        .await
    }

    async fn dispatch_udp_session(
        &mut self,
        session: Session,
        stream: TrojanAcceptedStream,
        responder: trojan::udp::TrojanInboundUdpResponder,
        auth: Option<zero_core::SessionAuth>,
    ) -> Result<(), Self::Error> {
        self.proxy
            .run_trojan_udp_relay(
                TcpRelayStream::new(stream.into_inner()),
                session,
                responder,
                auth,
                self.inbound_tag,
            )
            .await
    }
}

// Listener.

pub(crate) async fn run_trojan_listener_with_bound(
    proxy: &Proxy,
    request: TrojanInboundRequest,
    listener: zero_platform_tokio::TokioListener,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let TrojanInboundRequest {
        inbound,
        profile,
        tls_acceptor,
    } = request;

    let handler = TrojanInboundHandler {
        trojan_inbound: TrojanInbound,
        profile,
        tls_acceptor,
    };

    run_tcp_listener_loop(TcpListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "trojan",
        listener,
        shutdown,
        handler: move |engine: Proxy,
                       tag: String,
                       stream: zero_platform_tokio::TokioSocket,
                       source_addr: Option<std::net::SocketAddr>| {
            let handler = handler.clone();
            async move {
                let tls = match handler.tls_acceptor.accept(stream).await {
                    Ok(tls) => tls,
                    Err(e) => {
                        let e = EngineError::Io(io::Error::other(e));
                        if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                        {
                            warn!(?source_addr, %e, "trojan tls accept failed");
                        }
                        return;
                    }
                };
                let client = match handler
                    .profile
                    .accept_client(handler.trojan_inbound, AsyncSocketStream::new(tls))
                    .await
                {
                    Ok(client) => client,
                    Err(e) => {
                        let e = EngineError::from(e);
                        if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                            io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                        {
                            warn!(?source_addr, %e, "trojan auth failed");
                        }
                        return;
                    }
                };

                let mut bridge = TrojanAcceptedSessionBridge {
                    proxy: &engine,
                    handler: &handler,
                    inbound_tag: &tag,
                    source_addr,
                };
                let result = client.dispatch_with(&mut bridge).await;
                if let Err(e) = result {
                    if !matches!(&e, EngineError::Io(io) if matches!(io.kind(),
                        io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe))
                    {
                        warn!(?source_addr, %e, "trojan failed");
                    }
                }
            }
        },
    })
    .await
}
