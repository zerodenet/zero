//! Hysteria2 inbound: QUIC accept, HMAC auth, TCP stream dispatch.

#[path = "udp.rs"]
mod udp;

use async_trait::async_trait;
use std::io;
use tokio::sync::watch;
use tracing::{error, warn};
use zero_config::InboundConfig;
use zero_core::Session;
use zero_engine::EngineError;

use crate::runtime::inbound_protocol::{serve_inbound, InboundProtocol};
use crate::runtime::listener_loop::{run_quic_listener_loop, QuicListenerLoopRequest};
use crate::runtime::Proxy;
use crate::transport::Hysteria2Stream;

use super::request::Hysteria2InboundListenerRequest;

// Handler for individual TCP streams.

/// Handler for a single Hysteria2 TCP stream (QUIC bi-directional stream).
///
/// The QUIC connection lifecycle (auth, datagram loop) is managed by the
/// listener. This handler only deals with individual TCP streams.
#[derive(Clone)]
pub(crate) struct Hysteria2StreamHandler {
    acceptor: hysteria2::inbound::Hysteria2InboundTcpAcceptor,
}

#[async_trait]
impl InboundProtocol for Hysteria2StreamHandler {
    type ClientStream = Hysteria2Stream;

    async fn accept(
        &self,
        _stream: crate::transport::TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
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

// Listener (QUIC connection lifecycle).

pub(crate) async fn run_hysteria2_listener_with_bound(
    proxy: &Proxy,
    inbound: InboundConfig,
    request: Hysteria2InboundListenerRequest,
    bound: crate::protocol_registry::BoundInbound,
    shutdown: watch::Receiver<bool>,
) -> Result<(), EngineError> {
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
        acceptor: hysteria2::inbound::Hysteria2InboundTcpAcceptor::new(),
    };

    run_quic_listener_loop(QuicListenerLoopRequest {
        proxy,
        inbound_tag: inbound.tag,
        protocol_name: "hysteria2",
        listener: quic_inbound,
        shutdown,
        handler: move |engine: Proxy, tag: String, conn: quinn::Connection| {
            let profile = request.profile.clone();
            let handler = stream_handler.clone();
            async move {
                if let Err(error) =
                    handle_hysteria2_connection(&engine, conn, &tag, profile, &handler).await
                {
                    error!(error = %error, "hysteria2 connection error");
                }
            }
        },
    })
    .await
}

/// Handle a single Hysteria2 QUIC connection.
async fn handle_hysteria2_connection(
    proxy: &Proxy,
    conn: quinn::Connection,
    inbound_tag: &str,
    profile: hysteria2::inbound::Hysteria2InboundProfile,
    stream_handler: &Hysteria2StreamHandler,
) -> Result<(), EngineError> {
    let accepted = profile
        .accept_authenticated_quic_session(conn, Hysteria2Stream::new)
        .await?;

    accepted
        .dispatch_session_with_handlers(
            Hysteria2Stream::new,
            |conn, responder, tasks| {
                let tag = inbound_tag.to_owned();
                let engine = proxy.clone();
                tasks.spawn(async move {
                    udp::hysteria2_datagram_loop(conn, responder, tag, engine).await
                });
                async { Ok::<(), EngineError>(()) }
            },
            |session: Session, stream, tasks| {
                let engine = proxy.clone();
                let tag = inbound_tag.to_owned();
                let handler = stream_handler.clone();
                tasks.spawn(async move {
                    let _ = serve_inbound(&engine, session, stream, &handler, &tag, None).await;
                    Ok(())
                });
                async { Ok::<(), EngineError>(()) }
            },
            |result| async move {
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
                Ok::<(), EngineError>(())
            },
        )
        .await
}
