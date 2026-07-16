use std::time::Instant;

use zero_engine::EngineError;

use super::super::{FlowFailure, UdpDispatch};
use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::udp_flow::snapshot::UdpFlowSnapshot;

impl UdpDispatch {
    pub(super) fn fail_flow_with_msg(
        &mut self,
        flow: &UdpFlowSnapshot,
        started_at: Instant,
        stage: &'static str,
        msg: &str,
    ) {
        let error = EngineError::Io(std::io::Error::other(msg.to_string()));
        self.fail_flow(flow, started_at, stage, &error);
    }

    /// Record outbound bytes or fail the flow, for the common
    /// manager-based dispatch pattern in `forward_existing()`.
    pub(super) fn record_or_fail(
        &mut self,
        flow: &UdpFlowSnapshot,
        services: &UdpRuntimeServices,
        started_at: Instant,
        result: Result<usize, FlowFailure>,
    ) -> Result<(), EngineError> {
        match result {
            Ok(sent) => {
                services.record_session_outbound_tx(flow.session.id, sent as u64);
                Ok(())
            }
            Err(failure) => {
                self.fail_flow_with_msg(
                    flow,
                    started_at,
                    failure.stage,
                    &failure.error.to_string(),
                );
                Err(failure.error)
            }
        }
    }
}
