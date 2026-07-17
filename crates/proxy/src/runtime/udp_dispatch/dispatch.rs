use std::time::Instant;

use zero_core::{Network, Session};
use zero_engine::{EngineError, SessionOutcome};

use super::{FlowStartResult, UdpDispatch};
use crate::logging::{log_session_failed, log_session_finished};
use crate::runtime::pipe::UdpPipeInput;

impl UdpDispatch {
    /// Dispatch a UDP packet: route, select outbound, send.
    ///
    /// If a flow already exists for `(target, port, client_session_id)`, forwards
    /// the payload. Otherwise creates a new session, routes through the engine,
    /// and dispatches to the resolved outbound.
    pub(crate) async fn dispatch(&mut self, input: UdpPipeInput<'_>) -> Result<u64, EngineError> {
        if let Some(flow) = self
            .flows
            .snapshot(&input.target, input.port, input.client_session_id)
        {
            self.forward_existing(&flow, input.payload).await?;
            return Ok(flow.session.id);
        }

        self.start_new_routed_flow(input).await
    }

    async fn start_new_routed_flow(&mut self, input: UdpPipeInput<'_>) -> Result<u64, EngineError> {
        let runtime = self.runtime.clone();
        let mut session = Session::new(0, input.target, input.port, Network::Udp, input.protocol);
        if let Some(auth) = input.auth {
            session.auth = Some(auth.clone());
        }
        runtime.prepare_udp_session(&mut session, &self.inbound_tag);
        let mut session_handle = runtime.track_session(session.id);
        let started_at = Instant::now();
        runtime
            .services()
            .record_session_inbound_rx(session.id, input.payload.len() as u64);

        runtime.resolve_fake_ip_target(&mut session).await;
        let action = runtime.route_decision(&session);
        let resolved = match runtime.resolve_outbound(&action) {
            Ok(resolved) => resolved,
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
        runtime.log_session_accepted(&session, &action);

        match runtime
            .start_udp_resolved_outbound(self, &session, resolved, input.payload)
            .await
        {
            Ok(FlowStartResult::Flow { outbound, tx_bytes }) => {
                let session_id = session.id;
                session.outbound_tag = Some(outbound.tag().to_owned());
                runtime.set_session_outbound(&session);
                self.flows
                    .insert(session, session_handle, *outbound, input.client_session_id);
                runtime
                    .services()
                    .record_session_outbound_tx(session_id, tx_bytes);
                Ok(session_id)
            }
            Ok(FlowStartResult::Blocked { tag }) => {
                session.outbound_tag = Some(tag);
                runtime.set_session_outbound(&session);
                if let Some(record) = session_handle.finish(SessionOutcome::Blocked) {
                    log_session_finished(&record, None);
                }
                Ok(session.id)
            }
            Err(failure) => {
                let record = session_handle.finish(SessionOutcome::Failed);
                let stage = failure.stage;
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
                Err(failure.error)
            }
        }
    }
}
