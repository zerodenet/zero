use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::runtime::udp_associate::sessions::UdpFlowOutbound;

/// Result of starting a new UDP flow.
pub(crate) enum FlowStartResult {
    /// A new flow was established and tracked in `UdpSessionFlows`.
    Flow {
        outbound: Box<UdpFlowOutbound>,
        tx_bytes: u64,
    },
    /// A VLESS chain flow was established (tracked by the manager, not `UdpSessionFlows`).
    VlessFlow { session_id: u64, tag: String },
    /// A VMess UDP flow was established (tracked by the manager, not `UdpSessionFlows`).
    #[cfg(feature = "vmess")]
    VmessFlow { session_id: u64, tag: String },
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
