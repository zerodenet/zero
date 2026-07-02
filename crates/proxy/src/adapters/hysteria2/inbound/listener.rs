//! Hysteria2 inbound: QUIC accept, HMAC auth, TCP stream dispatch.

mod udp;

use async_trait::async_trait;
use hysteria2::{
    Hysteria2AcceptedQuicDispatcher, Hysteria2InboundProfile, Hysteria2InboundTcpAcceptor,
};
use std::io;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{error, warn};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::listener_loop::{run_quic_listener_loop, QuicListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::Hysteria2Stream;

#[derive(Debug)]
pub(crate) struct Hysteria2InboundRequest {
    pub(crate) inbound: InboundConfig,
    pub(crate) profile: Hysteria2InboundProfile,
}

// ── Handler for individual TCP streams ─────────────────────────────────

/// Handler for a single Hysteria2 TCP stream (QUIC bi-directional stream).
///
/// The QUIC connection lifecycle (auth, datagram loop) is managed by the
/// listener.  This handler only deals with individual TCP streams.
#[derive(Clone)]
pub(crate) struct Hysteria2StreamHandler {
    acceptor: Hysteria2InboundTcpAcceptor,
}

struct Hysteria2AcceptedQuicBridge<'a> {
    proxy: &'a Proxy,
    inbound_tag: &'a str,
    stream_handler: Hysteria2StreamHandler,
}

#[async_trait]
impl InboundProtocol for Hysteria2StreamHandler {
    type ClientStream = Hysteria2Stream;

    async fn accept(
        &self,
        _stream: crate::transport::TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        // Hysteria2 accept is handled inline by the listener; this is unused.
        Err(EngineError::Io(io::Error::new(
            io::ErrorKind::Unsupported,
            "Hysteria2 accept is handled by the listener",
        )))
    }

    async fn send_ok(&self, client: &mut Hysteria2Stream) -> Result<(), EngineError> {
        self.acceptor
            .send_ok(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut Hysteria2Stream) -> Result<(), EngineError> {
        let _ = self.acceptor.send_error(client, "blocked").await;
        Ok(())
    }

    async fn send_upstream_failure(&self, client: &mut Hysteria2Stream) -> Result<(), EngineError> {
        let _ = self.acceptor.send_error(client, "outbound failed").await;
        Ok(())
    }
}

impl Hysteria2AcceptedQuicDispatcher<Hysteria2Stream> for Hysteria2AcceptedQuicBridge<'_> {
    type Error = EngineError;

    async fn dispatch_udp_session(
        &mut self,
        conn: std::sync::Arc<quinn::Connection>,
        responder: hysteria2::Hysteria2InboundUdpResponder,
        tasks: &mut JoinSet<Result<(), Self::Error>>,
    ) -> Result<(), Self::Error> {
        let tag = self.inbound_tag.to_owned();
        let engine = self.proxy.clone();
        tasks.spawn(
            async move { Proxy::hysteria2_datagram_loop(conn, responder, tag, engine).await },
        );
        Ok(())
    }

    async fn dispatch_tcp_stream(
        &mut self,
        session: Session,
        stream: Hysteria2Stream,
        tasks: &mut JoinSet<Result<(), Self::Error>>,
    ) -> Result<(), Self::Error> {
        let engine = self.proxy.clone();
        let tag = self.inbound_tag.to_owned();
        let handler = self.stream_handler.clone();
        tasks.spawn(async move {
            let _ = serve_inbound(&engine, session, stream, &handler, &tag, None).await;
            Ok(())
        });
        Ok(())
    }

    async fn dispatch_stream_task_result(
        &mut self,
        result: Result<Result<(), Self::Error>, tokio::task::JoinError>,
    ) -> Result<(), Self::Error> {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                warn!(error = %error, "hysteria2 stream task failed");
            }
            Err(error) if !error.is_cancelled() => {
                error!(error = %error, "hysteria2 stream task panicked");
            }
            Err(_) => {}
        }
        Ok(())
    }
}

// ── Listener (QUIC connection lifecycle) ───────────────────────────────

pub(crate) async fn run_hysteria2_listener_with_bound(
    proxy: &Proxy,
    request: Hysteria2InboundRequest,
    bound: crate::protocol_registry::BoundInbound,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
    let Hysteria2InboundRequest { inbound, profile } = request;
    let quic_inbound = match bound {
        crate::protocol_registry::BoundInbound::Quic(e) => e,
        _ => {
            return Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hysteria2 listener requires QUIC transport",
            )))
        }
    };

    let stream_handler = Hysteria2StreamHandler {
        acceptor: Hysteria2InboundTcpAcceptor::new(),
    };

    run_quic_listener_loop(QuicListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "hysteria2",
        listener: quic_inbound,
        shutdown,
        handler: move |engine: Proxy, tag: String, conn: quinn::Connection| {
            let profile = profile.clone();
            let handler = stream_handler.clone();
            async move {
                if let Err(error) = engine
                    .handle_hysteria2_connection(conn, &tag, profile, &handler)
                    .await
                {
                    error!(error = %error, "hysteria2 connection error");
                }
            }
        },
    })
    .await
}

impl Proxy {
    /// Handle a single Hysteria2 QUIC connection.
    async fn handle_hysteria2_connection(
        &self,
        conn: quinn::Connection,
        inbound_tag: &str,
        profile: Hysteria2InboundProfile,
        stream_handler: &Hysteria2StreamHandler,
    ) -> Result<(), EngineError> {
        let accepted = profile
            .accept_authenticated_quic_session(conn, Hysteria2Stream::new)
            .await?;
        let mut bridge = Hysteria2AcceptedQuicBridge {
            proxy: self,
            inbound_tag,
            stream_handler: stream_handler.clone(),
        };
        accepted
            .dispatch_session(Hysteria2Stream::new, &mut bridge)
            .await
    }
}
