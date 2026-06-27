//! UDP flow forwarding for existing outbound connections.
//!
//! Handles re-dispatching packets on already-established UDP flows via
//! [`UdpDispatch::forward_existing`]. First-level dispatch is by
//! [`UdpPathCategory`]; per-protocol variants stay behind flow snapshot
//! accessors or `ProtocolUdpState`.
//!
//! | Category | Variants | Transport |
//! |----------|----------|-----------|
//! | `Direct` | `Direct` | Raw socket, no upstream manager |
//! | `Relay` | `Socks5` | UDP ASSOCIATE relay through control stream |
//! | `Datagram` | `Shadowsocks`, `Hysteria2` | Datagram encode/decode over socket or QUIC |
//! | `StreamPacket` | `Trojan`, `Mieru` | UDP packets over established stream |
//! | `PacketPathDatagram` | adapter-built packet-path snapshot | Datagram-over-carrier chain |

use std::time::Instant;

use zero_engine::EngineError;

use super::UdpDispatch;
use crate::runtime::udp_flow::sessions::{UdpFlowSnapshot, UdpPathCategory};
use crate::runtime::udp_helpers::send_direct_udp_packet;
use crate::runtime::Proxy;

impl UdpDispatch {
    /// Forward a packet to an existing flow.
    ///
    /// Dispatches by [`UdpPathCategory`] first, then by protocol-neutral flow
    /// accessors or `ProtocolUdpState`.
    pub(super) async fn forward_existing(
        &mut self,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let started_at = Instant::now();
        proxy.record_session_inbound_rx(flow.session.id, payload.len() as u64);

        match flow.outbound.path_category() {
            // Direct path.
            UdpPathCategory::Direct => {
                let Some(target_addr) = flow.outbound.direct_target_addr() else {
                    unreachable!("Direct category maps to Direct variant only");
                };
                match send_direct_udp_packet(&self.direct_socket, target_addr, payload).await {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        self.fail_flow(flow, started_at, "udp_direct_send", &error);
                        return Err(error);
                    }
                }
            }

            // Relay path.
            UdpPathCategory::Relay => {
                let Some(managed) = flow.outbound.relay_managed_flow() else {
                    unreachable!("Relay category maps to a managed relay flow");
                };
                match self
                    .forward_managed_relay_flow(proxy, flow, managed, payload)
                    .await
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        self.fail_flow(flow, started_at, "udp_upstream_send", &error);
                        return Err(error);
                    }
                }
            }

            UdpPathCategory::Datagram | UdpPathCategory::StreamPacket => {
                let result = self
                    .protocol_state
                    .forward_existing_protocol_flow(&mut self.chain_tasks, proxy, flow, payload)
                    .await;
                self.record_or_fail(flow, proxy, started_at, result)?;
            }

            UdpPathCategory::Cached => match self
                .protocol_state
                .send_existing_cached_flow(
                    &mut self.chain_tasks,
                    proxy,
                    &flow.session.target,
                    flow.session.port,
                    payload,
                )
                .await
            {
                Ok(Some(_session_id)) => {}
                Ok(None) => {
                    let error =
                        EngineError::Io(std::io::Error::other("cached UDP flow was dropped"));
                    self.fail_flow(flow, started_at, "udp_cached_send", &error);
                    return Err(error);
                }
                Err(error) => {
                    self.fail_flow(flow, started_at, "udp_cached_send", &error);
                    return Err(error);
                }
            },

            UdpPathCategory::PacketPathDatagram => {
                let result = self.forward_existing_packet_path_flow(flow, payload).await;
                self.record_or_fail(flow, proxy, started_at, result)?;
            }
        }

        Ok(())
    }

    fn fail_flow_with_msg(
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
    /// manager-based dispatch pattern in [`forward_existing()`].
    fn record_or_fail(
        &mut self,
        flow: &UdpFlowSnapshot,
        proxy: &Proxy,
        started_at: Instant,
        result: Result<usize, super::FlowFailure>,
    ) -> Result<(), EngineError> {
        match result {
            Ok(sent) => {
                proxy.record_session_outbound_tx(flow.session.id, sent as u64);
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
