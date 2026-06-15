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

#[cfg(feature = "shadowsocks")]
use super::packet_path_chain::{PacketPathCarrierParams, PacketPathChainParams};
use super::{
    H2UdpPeer, MieruUdpPeer, SsUdpPeer, TrojanUdpPeer, UdpDispatch, UdpFlowContext, UdpPacketRef,
    UdpPeerEndpoint,
};
use crate::runtime::udp_associate::sessions::{
    UdpFlowOutbound, UdpFlowSnapshot, UdpPacketPathCarrier, UdpPathCategory,
};
use crate::runtime::udp_helpers::send_direct_udp_packet;
use crate::runtime::Proxy;

#[cfg(feature = "shadowsocks")]
fn carrier_params(carrier: &UdpPacketPathCarrier) -> PacketPathCarrierParams<'_> {
    match carrier {
        #[cfg(feature = "socks5")]
        UdpPacketPathCarrier::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } => PacketPathCarrierParams::Socks5 {
            tag: tag.as_str(),
            server: server.as_str(),
            port: *port,
            username: username.as_deref(),
            password: password.as_deref(),
        },
        UdpPacketPathCarrier::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } => PacketPathCarrierParams::Shadowsocks {
            tag: tag.as_str(),
            server: server.as_str(),
            port: *port,
            password: password.as_str(),
            cipher: cipher.as_str(),
        },
        #[cfg(feature = "hysteria2")]
        UdpPacketPathCarrier::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
        } => PacketPathCarrierParams::Hysteria2 {
            tag: tag.as_str(),
            server: server.as_str(),
            port: *port,
            password: password.as_str(),
            client_fingerprint: client_fingerprint.as_deref(),
        },
    }
}

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
                    .send_socks5(
                        proxy,
                        tag,
                        server,
                        *port,
                        username.as_deref(),
                        password.as_deref(),
                        &flow.session,
                        payload,
                    )
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
                    tag: _,
                    server,
                    port,
                    password,
                    cipher,
                    packet_path_carrier,
                } => {
                    #[cfg(feature = "shadowsocks")]
                    let result = if let Some(carrier) = packet_path_carrier {
                        self.packet_path_manager
                            .send(
                                UdpFlowContext {
                                    chain_tasks: &mut self.chain_tasks,
                                    session_id: flow.session.id,
                                },
                                proxy,
                                &PacketPathChainParams {
                                    datagram_tag: "",
                                    carrier: carrier_params(carrier),
                                    datagram_server: server.as_str(),
                                    datagram_port: *port,
                                    datagram_password: password.as_str(),
                                    datagram_cipher: cipher.as_str(),
                                },
                                UdpPacketRef {
                                    target: &flow.session.target,
                                    port: flow.session.port,
                                    payload,
                                },
                            )
                            .await
                    } else {
                        self.ss_manager
                            .send(
                                UdpFlowContext {
                                    chain_tasks: &mut self.chain_tasks,
                                    session_id: flow.session.id,
                                },
                                proxy,
                                SsUdpPeer {
                                    endpoint: UdpPeerEndpoint {
                                        server: server.as_str(),
                                        port: *port,
                                    },
                                    password: password.as_str(),
                                    cipher: cipher.as_str(),
                                },
                                UdpPacketRef {
                                    target: &flow.session.target,
                                    port: flow.session.port,
                                    payload,
                                },
                            )
                            .await
                    };

                    self.record_or_fail(flow, proxy, started_at, result)?;
                }
                #[cfg(not(feature = "shadowsocks"))]
                UdpFlowOutbound::Shadowsocks { .. } => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Shadowsocks UDP outbound requires feature `shadowsocks`",
                    )));
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
                        .h2_manager
                        .send(
                            UdpFlowContext {
                                chain_tasks: &mut self.chain_tasks,
                                session_id: flow.session.id,
                            },
                            H2UdpPeer {
                                endpoint: UdpPeerEndpoint {
                                    server: server.as_str(),
                                    port: *port,
                                },
                                password: password.as_str(),
                                client_fingerprint: client_fingerprint.as_deref(),
                            },
                            UdpPacketRef {
                                target: &flow.session.target,
                                port: flow.session.port,
                                payload,
                            },
                        )
                        .await;
                    self.record_or_fail(flow, proxy, started_at, result)?;
                }
                #[cfg(not(feature = "hysteria2"))]
                UdpFlowOutbound::Hysteria2 { .. } => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Hysteria2 UDP outbound requires feature `hysteria2`",
                    )));
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
                        .trojan_manager
                        .send(
                            UdpFlowContext {
                                chain_tasks: &mut self.chain_tasks,
                                session_id: flow.session.id,
                            },
                            proxy,
                            &flow.session,
                            TrojanUdpPeer {
                                endpoint: UdpPeerEndpoint {
                                    server: server.as_str(),
                                    port: *port,
                                },
                                password: password.as_str(),
                                sni: sni.as_deref(),
                                insecure: *insecure,
                                client_fingerprint: client_fingerprint.as_deref(),
                                relay_chain: *relay_chain,
                            },
                            UdpPacketRef {
                                target: &flow.session.target,
                                port: flow.session.port,
                                payload,
                            },
                        )
                        .await;
                    self.record_or_fail(flow, proxy, started_at, result)?;
                }
                #[cfg(not(feature = "trojan"))]
                UdpFlowOutbound::Trojan { .. } => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Trojan UDP outbound requires feature `trojan`",
                    )));
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
                        .mieru_manager
                        .send(
                            UdpFlowContext {
                                chain_tasks: &mut self.chain_tasks,
                                session_id: flow.session.id,
                            },
                            proxy,
                            &flow.session,
                            MieruUdpPeer {
                                endpoint: UdpPeerEndpoint {
                                    server: server.as_str(),
                                    port: *port,
                                },
                                username: username.as_str(),
                                password: password.as_str(),
                                relay_chain: *relay_chain,
                            },
                            UdpPacketRef {
                                target: &flow.session.target,
                                port: flow.session.port,
                                payload,
                            },
                        )
                        .await;
                    self.record_or_fail(flow, proxy, started_at, result)?;
                }
                #[cfg(not(feature = "mieru"))]
                UdpFlowOutbound::Mieru { .. } => {
                    return Err(EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Mieru UDP outbound requires feature `mieru`",
                    )));
                }
                _ => unreachable!("StreamPacket category maps to Trojan or Mieru only"),
            },
        }

        Ok(())
    }
}
