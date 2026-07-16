use crate::runtime::udp_flow::sessions::UdpSessionFlows;
use crate::runtime::udp_flow::state::UdpFlowState;
use crate::runtime::udp_ingress::UdpIngressRuntime;
use zero_platform_tokio::TokioDatagramSocket;

/// Protocol-agnostic UDP dispatch state.
///
/// Owns per-session flow bookkeeping plus neutral registered-handler,
/// packet-path, and chain-task state.
/// Created per inbound UDP session/association.
pub(crate) struct UdpDispatch {
    pub(super) runtime: UdpIngressRuntime,
    pub(super) inbound_tag: String,
    pub(super) flows: UdpSessionFlows,
    /// Ephemeral UDP socket for direct outbound (sends to target, receives responses).
    pub(super) direct_socket: TokioDatagramSocket,
    /// Managed protocol, packet-path, and chain response state for this UDP session.
    pub(super) flow_state: UdpFlowState,
}
