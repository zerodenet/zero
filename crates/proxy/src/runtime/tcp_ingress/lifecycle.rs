use std::time::{Duration, Instant};

#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
use tokio::io::{AsyncRead, AsyncWrite};

#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
use zero_core::InboundClientResponse;
use zero_core::Session;
use zero_engine::{EngineError, SessionOutcome};
#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
use zero_traits::AsyncSocket;

#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
use super::contract::ClientResponseInboundProtocol;
use super::contract::InboundProtocol;
use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::runtime::Proxy;
use crate::transport::is_block_error;

#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
pub(crate) async fn serve_inbound_with_client_response<P, S>(
    proxy: &Proxy,
    session: Session,
    client: S,
    response_protocol: P,
    inbound_tag: &str,
    source_addr: Option<std::net::SocketAddr>,
) -> Result<(), EngineError>
where
    P: InboundClientResponse<S> + Send + Sync,
    S: AsyncRead + AsyncWrite + AsyncSocket + Unpin + Send,
{
    let protocol = ClientResponseInboundProtocol::new(response_protocol);
    serve_inbound(proxy, session, client, &protocol, inbound_tag, source_addr).await
}

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

    apply_url_rewrite(proxy, &mut session);
    apply_kernel_rate_limits(proxy, &mut session, inbound_tag);

    proxy.prepare_session(&mut session, inbound_tag, source_addr);

    let mut handle = proxy.track_session(session.id);
    let started_at = Instant::now();

    match TcpPipe::new(proxy)
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

            protocol.send_ok(&mut client).await?;

            let idle_secs = proxy
                .config
                .inbounds
                .iter()
                .find(|i| i.tag == inbound_tag)
                .and_then(|i| i.idle_timeout_secs)
                .unwrap_or(300);

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
    }
}

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
