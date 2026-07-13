use zero_engine::EngineError;

use super::outbound::UdpFlowOutbound;

/// Result of establishing a new persistent UDP flow.
pub(crate) enum FlowStartResult {
    Flow {
        outbound: Box<UdpFlowOutbound>,
        tx_bytes: u64,
    },
    Blocked {
        tag: String,
    },
}

/// Failure details produced while establishing or forwarding a UDP flow.
pub(crate) struct FlowFailure {
    pub(crate) stage: &'static str,
    pub(crate) error: EngineError,
    pub(crate) upstream: Option<(String, u16)>,
}
