//! UDP flow forwarding for existing outbound connections.
//!
//! Handles re-dispatching packets on already-established UDP flows via
//! [`UdpDispatch::forward_existing`]. First-level dispatch by
//! [`UdpPathCategory`], then by individual protocol variant within each
//! category:
//!
//! | Category | Variants | Transport |
//! |----------|----------|-----------|
//! | `Direct` | `Direct` | Raw socket, no upstream manager |
//! | `Relay` | `Socks5` | UDP ASSOCIATE relay through control stream |
//! | `Datagram` | `Shadowsocks`, `Hysteria2` | Datagram encode/decode over socket or QUIC |
//! | `StreamPacket` | `Trojan`, `Mieru` | UDP packets over established stream |

use std::time::Instant;

use zero_engine::EngineError;

use super::UdpDispatch;
use crate::runtime::udp_flow::sessions::{UdpFlowOutbound, UdpFlowSnapshot, UdpPathCategory};
use crate::runtime::udp_helpers::send_direct_udp_packet;
use crate::runtime::Proxy;

impl UdpDispatch {
    /// Forward a packet to an existing flow.
    ///
    /// Dispatches by [`UdpPathCategory`] first, then by protocol variant
    /// within each category. Adding a new protocol to an existing category
    /// only requires a new arm in the inner match; the outer category
    /// dispatch stays unchanged.
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
                let UdpFlowOutbound::Direct { target_addr, .. } = &flow.outbound else {
                    unreachable!("Direct category maps to Direct variant only");
                };
                match send_direct_udp_packet(&self.direct_socket, *target_addr, payload).await {
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
                let UdpFlowOutbound::Socks5 {
                    tag,
                    server,
                    port,
                    username,
                    password,
                } = &flow.outbound
                else {
                    unreachable!("Relay category maps to Socks5 variant only");
                };
                match self
                    .send_socks5(super::socks5_flow::Socks5UdpSend {
                        proxy,
                        tag,
                        server,
                        port: *port,
                        username: username.as_deref(),
                        password: password.as_deref(),
                        session: &flow.session,
                        payload,
                    })
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

            // Datagram path.
            UdpPathCategory::Datagram => match &flow.outbound {
                #[cfg(feature = "shadowsocks")]
                UdpFlowOutbound::Shadowsocks {
                    tag,
                    server,
                    port,
                    password,
                    cipher,
                    packet_path_carrier,
                } => {
                    #[cfg(feature = "shadowsocks")]
                    let result = if let Some(carrier) = packet_path_carrier {
                        self.protocol_state
                            .packet_path
                            .send_with_snapshot(
                                crate::protocol_runtime::udp::packet_path_traits::UdpFlowContext {
                                    chain_tasks: &mut self.chain_tasks,
                                    session_id: flow.session.id,
                                },
                                carrier,
                                tag.as_str(),
                                server.as_str(),
                                *port,
                                password.as_str(),
                                cipher.as_str(),
                                crate::protocol_runtime::udp::packet_path_traits::UdpPacketRef {
                                    target: &flow.session.target,
                                    port: flow.session.port,
                                    payload,
                                },
                            )
                            .await
                    } else {
                        self.protocol_state
                            .shadowsocks
                            .send_existing(crate::protocol_runtime::udp::SsSendExisting {
                                chain_tasks: &mut self.chain_tasks,
                                session_id: flow.session.id,
                                proxy,
                                server: server.as_str(),
                                port: *port,
                                password: password.as_str(),
                                cipher: cipher.as_str(),
                                target: &flow.session.target,
                                target_port: flow.session.port,
                                payload,
                            })
                            .await
                    };

                    self.record_or_fail(flow, proxy, started_at, result)?;
                }
                #[cfg(feature = "hysteria2")]
                UdpFlowOutbound::Hysteria2 {
                    tag: _,
                    server,
                    port,
                    password,
                    client_fingerprint,
                } => {
                    let result = self
                        .protocol_state
                        .hysteria2
                        .send_existing(crate::protocol_runtime::udp::H2SendExisting {
                            chain_tasks: &mut self.chain_tasks,
                            session_id: flow.session.id,
                            server: server.as_str(),
                            port: *port,
                            password: password.as_str(),
                            client_fingerprint: client_fingerprint.as_deref(),
                            target: &flow.session.target,
                            target_port: flow.session.port,
                            payload,
                        })
                        .await;
                    self.record_or_fail(flow, proxy, started_at, result)?;
                }
                _ => unreachable!("Datagram category maps to Shadowsocks or Hysteria2 only"),
            },

            // Stream packet path.
            UdpPathCategory::StreamPacket => match &flow.outbound {
                #[cfg(feature = "trojan")]
                UdpFlowOutbound::Trojan {
                    tag: _,
                    server,
                    port,
                    password,
                    sni,
                    insecure,
                    client_fingerprint,
                    relay_chain,
                } => {
                    let result = self
                        .protocol_state
                        .trojan
                        .send_existing(crate::protocol_runtime::udp::TrojanSendExisting {
                            chain_tasks: &mut self.chain_tasks,
                            session_id: flow.session.id,
                            proxy,
                            session: &flow.session,
                            server: server.as_str(),
                            port: *port,
                            password: password.as_str(),
                            sni: sni.as_deref(),
                            insecure: *insecure,
                            client_fingerprint: client_fingerprint.as_deref(),
                            relay_chain: *relay_chain,
                            target: &flow.session.target,
                            target_port: flow.session.port,
                            payload,
                        })
                        .await;
                    self.record_or_fail(flow, proxy, started_at, result)?;
                }
                #[cfg(feature = "mieru")]
                UdpFlowOutbound::Mieru {
                    tag: _,
                    server,
                    port,
                    username,
                    password,
                    relay_chain,
                } => {
                    let result = self
                        .protocol_state
                        .mieru
                        .send_existing(
                            &mut self.chain_tasks,
                            flow.session.id,
                            proxy,
                            &flow.session,
                            server.as_str(),
                            *port,
                            username.as_str(),
                            password.as_str(),
                            *relay_chain,
                            &flow.session.target,
                            flow.session.port,
                            payload,
                        )
                        .await;
                    self.record_or_fail(flow, proxy, started_at, result)?;
                }
                _ => unreachable!("StreamPacket category maps to Trojan or Mieru only"),
            },
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
