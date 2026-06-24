use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::runtime::udp_flow::outbound::UdpFlowOutbound;

/// Result of starting a new UDP flow.
pub(crate) enum FlowStartResult {
    /// A new flow was established and tracked in `UdpSessionFlows`.
    Flow {
        outbound: Box<UdpFlowOutbound>,
        tx_bytes: u64,
    },
    /// A protocol-managed flow was established outside `UdpSessionFlows`.
    ManagedFlow { session_id: u64, tag: String },
    /// The target was blocked.
    Blocked { tag: String },
}

/// Failure details for a flow start attempt.
pub(crate) struct FlowFailure {
    pub(crate) stage: &'static str,
    pub(crate) error: EngineError,
    pub(crate) upstream: Option<(String, u16)>,
}

pub(crate) enum UdpCandidate<'a> {
    Leaf(ResolvedLeafOutbound<'a>),
    Relay(Vec<ResolvedLeafOutbound<'a>>),
}
