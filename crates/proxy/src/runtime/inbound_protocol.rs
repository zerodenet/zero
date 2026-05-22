//! Kernel inbound protocol trait and unified session pipeline.
//!
//! The `InboundProtocol` trait is the boundary between protocol-specific
//! handshake/relay and the kernel's protocol-agnostic pipeline.  Every TCP
//! protocol implements this trait; the kernel provides `serve_inbound()` which
//! owns connection counting, rate limiting, routing, metering, and session
//! lifecycle — protocol handlers never touch those directly.

use std::time::Instant;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use zero_core::Session;
use zero_engine::{EngineError, SessionOutcome};

use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
use crate::runtime::Proxy;
use crate::transport::{is_block_error, relay_bidirectional_metered_throttled, TcpRelayStream};

// ── Trait ──────────────────────────────────────────────────────────────

/// Protocol-specific half of a TCP inbound handler.
///
/// Implementors provide handshake, response formatting, and data relay.
/// The kernel (`serve_inbound`) provides routing, rate limiting, session
/// tracking, and metering — the implementor never touches those.
#[async_trait]
pub(crate) trait InboundProtocol: Send + Sync {
    /// Stream type returned by the handshake (e.g. `TcpRelayStream`, QUIC stream).
    type ClientStream: AsyncRead + AsyncWrite + Unpin + Send;

    /// Handshake: authenticate, extract target address → `Session`.
    async fn accept(
        &self,
        stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError>;

    /// Notify the client that the tunnel has been established.
    async fn send_ok(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    /// Notify the client that the request was blocked.
    async fn send_blocked(&self, client: &mut Self::ClientStream) -> Result<(), EngineError>;

    /// Notify the client that the upstream is unreachable.
    async fn send_upstream_failure(
        &self,
        client: &mut Self::ClientStream,
    ) -> Result<(), EngineError>;

    /// Bidirectional relay between client and upstream.
    ///
    /// Default: raw TCP `io::copy` with optional rate limiting.
    /// Override for AEAD-framed (Shadowsocks) or QUIC-stream (Hysteria2) relays.
    async fn relay(
        &self,
        client: Self::ClientStream,
        upstream: TcpRelayStream,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Result<(), EngineError> {
        relay_bidirectional_metered_throttled(client, upstream, |_| {}, |_| {}, up_bps, down_bps)
            .await
            .map(|_| ())
            .map_err(|e| EngineError::Io(e))
    }
}

// ── Unified kernel pipeline ───────────────────────────────────────────

/// Single entry point for ALL TCP protocols (generic over the protocol type).
///
/// Owns every protocol-agnostic capability:
///   - connection counting / rate limiting (TODO — hook point)
///   - prepare → route → resolve → establish
///   - session tracking (track / finish)
///   - structured logging
///
/// Adding a new cross-cutting capability only requires changing THIS function.
pub(crate) async fn serve_inbound<P: InboundProtocol>(
    proxy: &Proxy,
    mut session: Session,
    mut client: P::ClientStream,
    protocol: &P,
    inbound_tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    // ── Kernel capability: connection counting / rate limiting hook ──
    // Future: check_connection_limit(inbound_tag)?;
    // Future: acquire_connection_slot(inbound_tag).await?;

    // Apply kernel rate policy: per-inbound defaults from config.
    // Per-user limits (if any) were already applied during protocol accept
    // — only fill in if not already set.
    apply_kernel_rate_limits(proxy, &mut session, inbound_tag);

    proxy.prepare_session(&mut session, inbound_tag, source_addr);

    let mut handle = proxy.track_session(session.id);
    let started_at = Instant::now();

    let result = match proxy.route_and_establish_tcp(&mut session).await {
        Ok(result) => {
            log_session_accepted(&session, &result.route_action, proxy.config.mode.kind());

            session.outbound_tag = Some(result.outbound_tag.clone());
            proxy.set_session_outbound(&session);

            let outcome = if result.is_direct {
                SessionOutcome::DirectRelayed
            } else {
                SessionOutcome::ChainedRelayed
            };
            let upstream_endpoint = result.upstream_endpoint.clone();

            // Protocol reply before relay
            protocol.send_ok(&mut client).await?;

            let relay_result = protocol
                .relay(client, result.upstream, session.up_bps, session.down_bps)
                .await;

            match relay_result {
                Ok(()) => {
                    if let Some(record) = handle.finish(outcome) {
                        log_session_finished(
                            &record,
                            upstream_endpoint
                                .as_ref()
                                .map(|(s, p)| (s.as_str(), *p)),
                        );
                    }
                    Ok(())
                }
                Err(error) => {
                    let record = handle.finish(SessionOutcome::Failed);
                    log_session_failed(
                        &session,
                        record.as_ref(),
                        "relay",
                        started_at.elapsed(),
                        &error,
                        upstream_endpoint
                            .as_ref()
                            .map(|(s, p)| (s.as_str(), *p)),
                    );
                    Err(error)
                }
            }
        }
        Err(error) if is_block_error(&error) => {
            let _ = protocol.send_blocked(&mut client).await;
            let record = handle.finish(SessionOutcome::Blocked);
            if let Some(ref record) = record {
                log_session_finished(record, None);
            }
            Ok(())
        }
        Err(error) => {
            let _ = protocol.send_upstream_failure(&mut client).await;
            let record = handle.finish(SessionOutcome::Failed);
            log_session_failed(
                &session,
                record.as_ref(),
                "route_or_establish",
                started_at.elapsed(),
                &error,
                None,
            );
            Err(error)
        }
    };

    result
}

/// Apply per-inbound rate limits from config as defaults.
///
/// Per-user limits (applied during protocol accept) take priority —
/// defaults only fill in if no per-user limit was set.
fn apply_kernel_rate_limits(proxy: &Proxy, session: &mut Session, inbound_tag: &str) {
    let Some(cfg) = proxy
        .config
        .inbounds
        .iter()
        .find(|i| i.tag == inbound_tag)
    else {
        return;
    };
    let (up, down) = cfg.protocol.rate_limits();
    if session.up_bps.is_none() {
        session.up_bps = up;
    }
    if session.down_bps.is_none() {
        session.down_bps = down;
    }
}
