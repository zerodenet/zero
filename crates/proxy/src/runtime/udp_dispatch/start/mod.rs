//! UDP flow start: new outbound establishment.
//!
//! Contains [`UdpDispatch::start_flow`] (single-hop) and
//! [`UdpDispatch::start_relay_flow`] (multi-hop chain). Packet-path carrier
//! and datagram roles are resolved via `ProtocolInventory`; there is no
//! per-protocol match here.

use super::{FlowFailure, FlowStartResult, UdpCandidate, UdpDispatch};
use crate::runtime::Proxy;
use zero_core::Session;

impl UdpDispatch {
    /// Start a new UDP flow by dispatching to the resolved outbound.
    pub(super) async fn start_flow(
        &mut self,
        proxy: &Proxy,
        candidate: UdpCandidate<'_>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let candidate = match candidate {
            UdpCandidate::Leaf(candidate) => candidate,
            UdpCandidate::Relay(chain) => {
                return self.start_relay_flow(proxy, chain, session, payload).await;
            }
        };

        // Block is kernel-level (no adapter owns it): reject immediately.
        // Direct and every proxy protocol go through the adapter registry:
        // adding a protocol = register an adapter, zero changes here.
        let runtime = proxy
            .protocols
            .outbound_leaf_runtime(&candidate)
            .map_err(|error| FlowFailure {
                stage: "outbound_leaf_runtime",
                error,
                upstream: None,
            })?;
        if matches!(
            runtime.tcp_path,
            crate::runtime::orchestration::TcpPathCategory::Block
        ) {
            return Ok(FlowStartResult::Blocked {
                tag: runtime.kernel_tag.unwrap_or("block").to_string(),
            });
        }

        proxy
            .protocols
            .start_udp_leaf_flow(self, proxy, session, &candidate, payload)
            .await
    }
}

mod relay;
