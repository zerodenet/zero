use std::time::Instant;

#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
use tokio::io::{AsyncRead, AsyncWrite};

use zero_config::RuntimeConfig;
#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
use zero_core::InboundClientResponse;
use zero_core::Session;
use zero_engine::{EngineError, SessionOutcome};
#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
use zero_traits::AsyncSocket;

#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
use super::contract::ClientResponseInboundProtocol;
use super::contract::InboundProtocol;
use super::runtime::TcpIngressRuntime;
use crate::logging::{log_session_failed, log_session_finished};
use crate::runtime::pipe::{KernelPipe, TcpPipe, TcpPipeInput};
use crate::transport::is_block_error;

#[cfg(any(feature = "socks5", feature = "hysteria2", feature = "mieru"))]
pub(crate) async fn serve_inbound_with_client_response<P, S>(
    runtime: &TcpIngressRuntime,
    session: Session,
    client: S,
    response_protocol: P,
) -> Result<(), EngineError>
where
    P: InboundClientResponse<S> + Send + Sync,
    S: AsyncRead + AsyncWrite + AsyncSocket + Unpin + Send,
{
    let protocol = ClientResponseInboundProtocol::new(response_protocol);
    serve_inbound(runtime, session, client, &protocol).await
}

pub(crate) async fn serve_inbound<P: InboundProtocol>(
    runtime: &TcpIngressRuntime,
    session: Session,
    client: P::ClientStream,
    protocol: &P,
) -> Result<(), EngineError> {
    let mut session = session;
    let mut client = client;

    runtime.apply_url_rewrite(&mut session);
    runtime.apply_kernel_rate_limits(&mut session);
    runtime.prepare_session(&mut session);

    let mut handle = runtime.track_session(session.id);
    let started_at = Instant::now();

    match TcpPipe::new(runtime)
        .dispatch(TcpPipeInput {
            session: &mut session,
        })
        .await
    {
        Ok(result) => {
            runtime.log_session_accepted(&session, &result.route_action);

            session.outbound_tag = Some(result.outbound_tag.clone());
            runtime.set_session_outbound(&session);

            let outcome = if result.is_direct {
                SessionOutcome::DirectRelayed
            } else {
                SessionOutcome::ChainedRelayed
            };
            let upstream_endpoint = result.upstream_endpoint.clone();

            protocol.send_ok(&mut client).await?;

            let relay_result = tokio::time::timeout(
                runtime.idle_timeout(),
                protocol.relay(
                    client,
                    result.upstream,
                    runtime.runtime_services(),
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

pub(crate) fn apply_kernel_rate_limits_from_config(
    config: &RuntimeConfig,
    session: &mut Session,
    inbound_tag: &str,
) {
    let Some(cfg) = config.inbounds.iter().find(|i| i.tag == inbound_tag) else {
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
