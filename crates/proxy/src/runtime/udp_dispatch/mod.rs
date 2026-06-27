//! Generic UDP dispatch: protocol-agnostic routing and outbound dispatch.
//!
//! [`UdpDispatch`] is the UDP pipe state machine.
//! Inbound protocols create one dispatcher per UDP association/session, submit
//! packets through [`crate::runtime::pipe::UdpPipe`], and poll this dispatcher
//! for responses to deliver to the client.
//!
//! # Module layout
//!
//! - [`forward`]: re-dispatch packets on existing outbound flows
//! - [`start`]: establish new outbound flows (single-hop and relay chains)
//! - [`crate::runtime::udp_flow::registered`]: registered protocol UDP handlers
//! - [`packet_path_chain`]: generic datagram-over-packet-path manager for
//!   relay chains (Shadowsocks -> Shadowsocks, SOCKS5 -> Shadowsocks, etc.)
//!
//! # Supported outbounds
//!
//! All outbound types: direct, block, socks5, vless, shadowsocks, hysteria2,
//! trojan, mieru.
//!
//! # UDP relay chain model
//!
//! The relay chain model is:
//!
//! ```text
//! previous hop provides a packet path (send/recv raw payloads)
//! next hop encodes its protocol datagram through that path
//! ```
//!
//! Adding new datagram-over-packet-path combinations requires implementing
//! [`UdpPacketPath`] and [`DatagramCodec`], not creating protocol-pair modules.
//!
//! # Usage
//!
//! ```ignore
//! let mut dispatch = UdpDispatch::new("inbound-tag").await?;
//!
//! // For each incoming packet:
//! UdpPipe::new(proxy, &mut dispatch).dispatch(input).await?;
//!
//! // Poll for responses in a select loop:
//! select! {
//!     recv = dispatch.direct_socket().recv_from_addr(&mut buf) => { /* direct response */ }
//!     resp = dispatch.poll_chain_response() => { /* chain response */ }
//!     // ...
//! }
//!
//! // Cleanup:
//! for completed in dispatch.finish_all() {
//!     log_completed_udp_flow(completed);
//! }
//! ```

use std::time::Instant;

use crate::logging::log_session_failed;
use crate::runtime::udp_flow::sessions::{UdpFlowSnapshot, UdpSessionFlows};
use crate::runtime::udp_flow::state::UdpFlowState;
use zero_engine::{EngineError, SessionOutcome};
use zero_platform_tokio::TokioDatagramSocket;

// Sub-module declarations.

mod dispatch;
mod forward;
mod lifecycle;
mod managed;
mod packet_path;
mod start;
mod types;

// Re-exports.

pub(crate) use managed::{ManagedUdpOutboundKind, ManagedUdpSend};
pub(crate) use types::{FlowFailure, FlowStartResult, UdpCandidate};

// UdpDispatch.

/// Protocol-agnostic UDP dispatch state.
///
/// Owns generic UDP dispatch state and a protocol runtime state bundle.
/// Created per inbound UDP session/association.
pub(crate) struct UdpDispatch {
    inbound_tag: String,
    flows: UdpSessionFlows,
    /// Ephemeral UDP socket for direct outbound (sends to target, receives responses).
    direct_socket: TokioDatagramSocket,
    /// Managed protocol, packet-path, and chain response state for this UDP session.
    flow_state: UdpFlowState,
}

impl UdpDispatch {
    // Failure helpers.

    fn fail_flow(
        &mut self,
        flow: &UdpFlowSnapshot,
        started_at: Instant,
        stage: &'static str,
        error: &EngineError,
    ) {
        if let Some(completed) = self.flows.finish(
            &flow.session.target,
            flow.session.port,
            flow.client_session_id,
            SessionOutcome::Failed,
        ) {
            log_session_failed(
                &flow.session,
                Some(&completed.record),
                stage,
                started_at.elapsed(),
                error,
                None,
            );
        } else {
            log_session_failed(
                &flow.session,
                None,
                stage,
                started_at.elapsed(),
                error,
                None,
            );
        }
    }
}
