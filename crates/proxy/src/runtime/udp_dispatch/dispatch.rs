use std::time::Instant;

use zero_core::{Network, Session};
use zero_engine::{EngineError, ResolvedOutbound, SessionOutcome};

use super::{FlowStartResult, UdpCandidate, UdpDispatch};
use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
use crate::runtime::inbound_protocol::apply_kernel_rate_limits;
use crate::runtime::pipe::UdpPipeInput;
use crate::runtime::Proxy;

impl UdpDispatch {
    /// Dispatch a UDP packet: route, select outbound, send.
    ///
    /// If a flow already exists for `(target, port, client_session_id)`
    /// (including VLESS chain connections cached in the manager), forwards the
    /// payload. Otherwise creates a new session, routes through the engine, and
    /// dispatches to the resolved outbound.
    pub(crate) async fn dispatch(
        &mut self,
        proxy: &Proxy,
        input: UdpPipeInput<'_>,
    ) -> Result<u64, EngineError> {
        if let Some(session_id) = self
            .protocol_state
            .send_existing_cached_flow(
                &mut self.chain_tasks,
                proxy,
                &input.target,
                input.port,
                input.payload,
            )
            .await?
        {
            return Ok(session_id);
        }

        if let Some(flow) = self
            .flows
            .snapshot(&input.target, input.port, input.client_session_id)
        {
            self.forward_existing(proxy, &flow, input.payload).await?;
            return Ok(flow.session.id);
        }

        self.start_new_routed_flow(proxy, input).await
    }

    async fn start_new_routed_flow(
        &mut self,
        proxy: &Proxy,
        input: UdpPipeInput<'_>,
    ) -> Result<u64, EngineError> {
        let mut session = Session::new(0, input.target, input.port, Network::Udp, input.protocol);
        if let Some(auth) = input.auth {
            session.auth = Some(auth.clone());
        }
        proxy.prepare_session(&mut session, &self.inbound_tag, None);
        apply_kernel_rate_limits(proxy, &mut session, &self.inbound_tag);
        let mut session_handle = proxy.track_session(session.id);
        let started_at = Instant::now();
        proxy.record_session_inbound_rx(session.id, input.payload.len() as u64);

        proxy.resolve_fake_ip_target(&mut session).await;
        let action = proxy.route_decision(&session);
        let resolved = match proxy.resolve_outbound(&action) {
            Ok((resolved, _plan)) => resolved,
            Err(error) => {
                let record = session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &session,
                    record.as_ref(),
                    "resolve_outbound",
                    started_at.elapsed(),
                    &error,
                    None,
                );
                return Err(error);
            }
        };
        log_session_accepted(&session, &action, proxy.config.mode.kind());

        let candidates = match resolved {
            ResolvedOutbound::Single(candidate) => vec![UdpCandidate::Leaf(candidate)],
            ResolvedOutbound::Fallback { candidates } => {
                candidates.into_iter().map(UdpCandidate::Leaf).collect()
            }
            ResolvedOutbound::Relay { chain } => vec![UdpCandidate::Relay(chain)],
        };
        let is_fallback = candidates.len() > 1;
        let mut last_failure = None;

        for candidate in candidates {
            match self
                .start_flow(proxy, candidate, &session, input.payload)
                .await
            {
                Ok(FlowStartResult::Flow { outbound, tx_bytes }) => {
                    let session_id = session.id;
                    session.outbound_tag = Some(outbound.tag().to_owned());
                    proxy.set_session_outbound(&session);
                    self.flows
                        .insert(session, session_handle, *outbound, input.client_session_id);
                    proxy.record_session_outbound_tx(session_id, tx_bytes);
                    return Ok(session_id);
                }
                Ok(FlowStartResult::VlessFlow { session_id, tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    self.vless_handles.insert(
                        (session.target.clone(), session.port),
                        (session, session_handle),
                    );
                    proxy.record_session_outbound_tx(session_id, input.payload.len() as u64);
                    return Ok(session_id);
                }
                #[cfg(feature = "vmess")]
                Ok(FlowStartResult::VmessFlow { session_id, tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    self.vmess_handles.insert(
                        (session.target.clone(), session.port),
                        (session, session_handle),
                    );
                    proxy.record_session_outbound_tx(session_id, input.payload.len() as u64);
                    return Ok(session_id);
                }
                Ok(FlowStartResult::Blocked { tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    if let Some(record) = session_handle.finish(SessionOutcome::Blocked) {
                        log_session_finished(&record, None);
                    }
                    return Ok(session.id);
                }
                Err(failure) => {
                    last_failure = Some(failure);
                }
            }
        }

        let record = session_handle.finish(SessionOutcome::Failed);
        if let Some(failure) = last_failure {
            let stage = if is_fallback {
                "fallback_exhausted"
            } else {
                failure.stage
            };
            log_session_failed(
                &session,
                record.as_ref(),
                stage,
                started_at.elapsed(),
                &failure.error,
                failure
                    .upstream
                    .as_ref()
                    .map(|(server, port)| (server.as_str(), *port)),
            );
            return Err(failure.error);
        }

        let error = EngineError::Io(std::io::Error::other("all fallback outbounds failed"));
        log_session_failed(
            &session,
            record.as_ref(),
            "fallback_exhausted",
            started_at.elapsed(),
            &error,
            None,
        );
        Err(error)
    }
}
