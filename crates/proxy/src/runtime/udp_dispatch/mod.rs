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
//! - [`crate::protocol_runtime::udp`]: protocol-specific UDP managers
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

use std::collections::HashMap;
use std::time::Instant;

use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;

use crate::logging::{log_session_failed, log_session_finished};
use crate::runtime::udp_flow::sessions::{UdpFlowSnapshot, UdpSessionFlows};
use crate::runtime::Proxy;
use zero_core::{Address, Session};
use zero_engine::{EngineError, SessionHandle, SessionOutcome};
use zero_platform_tokio::TokioDatagramSocket;

// Sub-module declarations.

mod dispatch;
mod forward;
mod lifecycle;
mod protocol_flows;
mod socks5_flow;
mod start;
mod types;

// Re-exports.

#[cfg(all(feature = "shadowsocks", feature = "socks5"))]
pub(crate) use crate::protocol_runtime::socks5_udp::build_socks5_packet_path;
#[cfg(all(feature = "shadowsocks", feature = "hysteria2"))]
pub(crate) use crate::protocol_runtime::udp::build_hysteria2_packet_path;
#[cfg(feature = "shadowsocks")]
pub(crate) use crate::protocol_runtime::udp::build_shadowsocks_packet_path;
pub(crate) use crate::protocol_runtime::udp::ChainTask;
use crate::protocol_runtime::udp::ProtocolUdpState;
#[cfg(feature = "shadowsocks")]
pub(crate) use crate::protocol_runtime::udp::{
    PacketPathCarrier, PacketPathCarrierDescriptor, UdpDatagramSource,
};
#[cfg(feature = "mieru")]
pub(crate) use protocol_flows::MieruUdpRelayFlow;
#[cfg(feature = "shadowsocks")]
pub(crate) use protocol_flows::ShadowsocksUdpFlow;
#[cfg(feature = "vless")]
pub(crate) use protocol_flows::{VlessUdpFlow, VlessUdpRelayFinalHop, VlessUdpRelayTwoStream};
#[cfg(feature = "vmess")]
pub(crate) use protocol_flows::{VmessUdpFlow, VmessUdpRelayFlow};
pub(crate) use socks5_flow::Socks5UdpSend;
pub(crate) use types::{FlowFailure, FlowStartResult, UdpCandidate};

// UdpDispatch.

/// Protocol-agnostic UDP dispatch state.
///
/// Owns generic UDP dispatch state and a protocol runtime state bundle.
/// Created per inbound UDP session/association.
pub(crate) struct UdpDispatch {
    pub(crate) inbound_tag: String,
    pub(crate) flows: UdpSessionFlows,
    /// Ephemeral UDP socket for direct outbound (sends to target, receives responses).
    pub(crate) direct_socket: TokioDatagramSocket,
    /// SOCKS5 upstream association (shared across all flows in this session).
    pub(crate) socks5_upstream:
        Option<crate::protocol_runtime::socks5_udp::ActiveUpstreamSocks5UdpAssociation>,
    pub(crate) socks5_idle_deadline: Option<TokioInstant>,
    /// Protocol-specific UDP managers.
    protocol_state: ProtocolUdpState,
    /// Session handles for VLESS chain flows. These are not tracked by
    /// [`UdpSessionFlows`] because the VLESS manager owns the per-target
    /// upstream connections. We store handles here so `finish_all()` can
    /// properly complete them.
    vless_handles: HashMap<(Address, u16), (Session, SessionHandle)>,
    /// Session handles for VMess UDP flows owned by the VMess manager.
    #[cfg(feature = "vmess")]
    vmess_handles: HashMap<(Address, u16), (Session, SessionHandle)>,
    /// Unified JoinSet for chain-outbound (SS/H2/Trojan/Mieru/VLESS)
    /// response bridge tasks. Polled by [`poll_chain_response`].
    pub(crate) chain_tasks: JoinSet<ChainTask>,
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
