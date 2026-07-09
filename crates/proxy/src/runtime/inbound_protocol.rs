//! Kernel inbound protocol trait and unified session pipeline.
//!
//! The `InboundProtocol` trait is the boundary between protocol-specific
//! handshake/relay and the kernel's protocol-agnostic pipeline.  Every TCP
//! protocol implements this trait; the kernel provides `serve_inbound()` which
//! owns connection counting, rate limiting, routing, metering, and session
//! lifecycle — protocol handlers never touch those directly.

use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use zero_core::{InboundClientResponse, Session};
use zero_engine::{EngineError, SessionOutcome};
use zero_traits::AsyncSocket;

use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::runtime::Proxy;
use crate::transport::{is_block_error, relay_bidirectional_metered_throttled, TcpRelayStream};

pub(crate) fn record_tcp_upload(proxy: &Proxy, session_id: u64, bytes: u64) {
    proxy.record_session_inbound_rx(session_id, bytes);
    proxy.record_session_outbound_tx(session_id, bytes);
}

pub(crate) fn record_tcp_download(proxy: &Proxy, session_id: u64, bytes: u64) {
    proxy.record_session_outbound_rx(session_id, bytes);
    proxy.record_session_inbound_tx(session_id, bytes);
}

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
        proxy: &Proxy,
        session_id: u64,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Result<(), EngineError> {
        relay_bidirectional_metered_throttled(
            client,
            upstream,
            |bytes| {
                record_tcp_upload(proxy, session_id, bytes);
            },
            |bytes| {
                record_tcp_download(proxy, session_id, bytes);
            },
            up_bps,
            down_bps,
        )
        .await
        .map(|_| ())
        .map_err(EngineError::Io)
    }
}

pub(crate) struct ClientResponseInboundProtocol<P, S> {
    protocol: P,
    _stream: core::marker::PhantomData<fn() -> S>,
}

impl<P, S> ClientResponseInboundProtocol<P, S> {
    pub(crate) const fn new(protocol: P) -> Self {
        Self {
            protocol,
            _stream: core::marker::PhantomData,
        }
    }
}

impl<P, S> Clone for ClientResponseInboundProtocol<P, S>
where
    P: Clone,
{
    fn clone(&self) -> Self {
        Self {
            protocol: self.protocol.clone(),
            _stream: core::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<P, S> InboundProtocol for ClientResponseInboundProtocol<P, S>
where
    P: InboundClientResponse<S> + Send + Sync,
    S: AsyncRead + AsyncWrite + AsyncSocket + Unpin + Send,
{
    type ClientStream = S;

    async fn accept(
        &self,
        _stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        unreachable!("accept is handled before serve_inbound dispatch")
    }

    async fn send_ok(&self, client: &mut Self::ClientStream) -> Result<(), EngineError> {
        self.protocol
            .send_ok(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_blocked(&self, client: &mut Self::ClientStream) -> Result<(), EngineError> {
        self.protocol
            .send_blocked(client)
            .await
            .map_err(EngineError::from)
    }

    async fn send_upstream_failure(
        &self,
        client: &mut Self::ClientStream,
    ) -> Result<(), EngineError> {
        self.protocol
            .send_upstream_failure(client)
            .await
            .map_err(EngineError::from)
    }
}

#[derive(Clone, Copy, Default)]
pub(crate) struct NoClientResponseInboundProtocol;

#[async_trait]
impl InboundProtocol for NoClientResponseInboundProtocol {
    type ClientStream = TcpRelayStream;

    async fn accept(
        &self,
        _stream: TcpRelayStream,
    ) -> Result<(Session, Self::ClientStream), EngineError> {
        unreachable!("accept is handled before serve_inbound dispatch")
    }

    async fn send_ok(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_blocked(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }

    async fn send_upstream_failure(&self, _client: &mut TcpRelayStream) -> Result<(), EngineError> {
        Ok(())
    }
}

// ── Unified kernel pipeline ───────────────────────────────────────────

/// Single entry point for ALL TCP protocols (generic over the protocol type).
///
/// Owns every protocol-agnostic capability:
///   - per-inbound rate defaults and session admission metadata
///   - prepare → route → resolve → establish
///   - session tracking (track / finish)
///   - structured logging
///
/// Adding a new cross-cutting capability only requires changing THIS function.
pub(crate) async fn serve_inbound<P: InboundProtocol>(
    proxy: &Proxy,
    session: Session,
    client: P::ClientStream,
    protocol: &P,
    inbound_tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError> {
    let mut session = session;
    let mut client = client;

    // ── Kernel primitive: URL rewrite ──
    apply_url_rewrite(proxy, &mut session);

    // Apply kernel rate policy: per-inbound defaults from config.
    apply_kernel_rate_limits(proxy, &mut session, inbound_tag);

    proxy.prepare_session(&mut session, inbound_tag, source_addr);

    let mut handle = proxy.track_session(session.id);
    let started_at = Instant::now();

    let result = match TcpPipe::new(proxy)
        .dispatch(TcpPipeInput {
            session: &mut session,
        })
        .await
    {
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

            // ── Kernel primitive: idle timeout ──
            let idle_secs = proxy
                .config
                .inbounds
                .iter()
                .find(|i| i.tag == inbound_tag)
                .and_then(|i| i.idle_timeout_secs)
                .unwrap_or(300); // kernel default: 5 min

            let relay_result = tokio::time::timeout(
                Duration::from_secs(idle_secs),
                protocol.relay(
                    client,
                    result.upstream,
                    proxy,
                    session.id,
                    session.up_bps,
                    session.down_bps,
                ),
            )
            .await;

            match relay_result {
                Ok(Ok(())) => {
                    if let Some(record) = handle.finish(outcome) {
                        log_session_finished(
                            &record,
                            upstream_endpoint.as_ref().map(|(s, p)| (s.as_str(), *p)),
                        );
                    }
                    Ok(())
                }
                Ok(Err(error)) => {
                    let record = handle.finish_with_reason(
                        SessionOutcome::Failed,
                        Some("upstream_error".to_owned()),
                    );
                    log_session_failed(
                        &session,
                        record.as_ref(),
                        "relay",
                        started_at.elapsed(),
                        &error,
                        upstream_endpoint.as_ref().map(|(s, p)| (s.as_str(), *p)),
                    );
                    Err(error)
                }
                Err(_elapsed) => {
                    // Idle timeout — clean finish, not an error.
                    if let Some(record) =
                        handle.finish_with_reason(outcome, Some("idle_timeout".to_owned()))
                    {
                        log_session_finished(
                            &record,
                            upstream_endpoint.as_ref().map(|(s, p)| (s.as_str(), *p)),
                        );
                    }
                    Ok(())
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
            let record = handle
                .finish_with_reason(SessionOutcome::Failed, Some("upstream_error".to_owned()));
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

/// Apply URL rewrite rules from route config before routing.
///
/// Matches session target domain against configured rewrite rules
/// (exact `from` or `from_regex` patterns) and replaces with `to`.
fn apply_url_rewrite(proxy: &Proxy, session: &mut Session) {
    let rules = &proxy.config.route.url_rewrite;
    if rules.is_empty() {
        return;
    }
    let zero_core::Address::Domain(ref domain) = session.target else {
        return;
    };
    for rule in rules {
        if let Some(ref from) = rule.from {
            if from == domain {
                session.target = zero_core::Address::Domain(rule.to.clone());
                return;
            }
        }
        if let Some(ref pattern) = &rule.from_regex {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(domain) {
                    let result = re.replace(domain, &rule.to);
                    session.target = zero_core::Address::Domain(result.to_string());
                    return;
                }
            }
        }
    }
}

/// Apply per-inbound rate limits from config as defaults.
///
/// Per-user limits (applied during protocol accept) take priority —
/// defaults only fill in if no per-user limit was set.
pub(crate) fn apply_kernel_rate_limits(proxy: &Proxy, session: &mut Session, inbound_tag: &str) {
    let Some(cfg) = proxy.config.inbounds.iter().find(|i| i.tag == inbound_tag) else {
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
