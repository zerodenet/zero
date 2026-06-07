//! UDP flow start — new outbound establishment.
//!
//! Contains [`UdpDispatch::start_flow`] (single-hop) and
//! [`UdpDispatch::start_relay_flow`] (multi-hop chain) for establishing new
//! UDP outbound connections, plus the chain resolution function
//! [`resolve_udp_packet_path_chain`].

use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
use super::packet_path_chain::PacketPathChainParams;
use super::{FlowFailure, FlowStartResult, UdpCandidate, UdpDispatch};
use crate::runtime::udp_associate::sessions::{UdpFlowOutbound, UdpPacketPathCarrier};
use crate::runtime::vless_udp::{establish_vless_udp_upstream_over_stream, VlessUdpTransport};
use crate::runtime::Proxy;

// ── Chain resolution ─────────────────────────────────────────────────

/// Resolve a relay chain into packet-path + datagram parameters.
///
/// Returns `Some` when the chain matches the "packet path carrier → datagram
/// protocol" pattern. Currently recognises `[SOCKS5, Shadowsocks]`. Adding
/// new combinations only requires extending this function and implementing
/// [`UdpPacketPath`] + [`DatagramCodec`] — no new protocol-pair modules.
#[cfg(all(feature = "socks5", feature = "shadowsocks"))]
fn resolve_udp_packet_path_chain<'a>(
    chain: &[ResolvedLeafOutbound<'a>],
) -> Option<PacketPathChainParams<'a>> {
    match chain {
        [ResolvedLeafOutbound::Socks5 {
            tag: carrier_tag,
            server: carrier_server,
            port: carrier_port,
            username: carrier_username,
            password: carrier_password,
        }, ResolvedLeafOutbound::Shadowsocks {
            tag: datagram_tag,
            server: datagram_server,
            port: datagram_port,
            password: datagram_password,
            cipher: datagram_cipher,
        }] => Some(PacketPathChainParams {
            datagram_tag,
            carrier_tag,
            carrier_server,
            carrier_port: *carrier_port,
            carrier_username: *carrier_username,
            carrier_password: *carrier_password,
            datagram_server,
            datagram_port: *datagram_port,
            datagram_password,
            datagram_cipher,
        }),
        _ => None,
    }
}

// ── impl UdpDispatch ─────────────────────────────────────────────────

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

        match candidate {
            ResolvedLeafOutbound::Direct { tag } => {
                let target_addr = proxy
                    .protocols
                    .direct_outbound
                    .resolve_target_addr(session, proxy.resolver.as_ref())
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "resolve_udp_target",
                        error: error.into(),
                        upstream: None,
                    })?;

                let sent = self
                    .direct_socket
                    .send_to_addr(payload, target_addr)
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_direct_send",
                        error: error.into(),
                        upstream: None,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Direct {
                        tag: tag.unwrap_or("direct").to_owned(),
                        target_addr,
                    },
                    tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Block { tag } => Ok(FlowStartResult::Blocked {
                tag: tag.unwrap_or("block").to_owned(),
            }),
            ResolvedLeafOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let sent = self
                    .send_socks5(
                        proxy, tag, server, port, username, password, session, payload,
                    )
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_upstream_send",
                        error,
                        upstream: Some((server.to_owned(), port)),
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Socks5 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.map(ToOwned::to_owned),
                        password: password.map(ToOwned::to_owned),
                    },
                    tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Vless {
                tag,
                server,
                port,
                id,
                tls,
                reality,
                ws,
                grpc,
                h2,
                http_upgrade,
                split_http,
                quic,
                ..
            } => {
                let transport = VlessUdpTransport {
                    tls,
                    reality,
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    quic,
                };
                let session_id = session.id;
                let tag_owned = tag.to_owned();
                self.vless_manager
                    .get_or_create_upstream(
                        &mut self.chain_tasks,
                        proxy,
                        session,
                        session.target.clone(),
                        session.port,
                        server.to_owned(),
                        port,
                        id.to_owned(),
                        payload.to_vec(),
                        Some(&transport),
                    )
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_vless_upstream",
                        error,
                        upstream: Some((server.to_owned(), port)),
                    })?;

                Ok(FlowStartResult::VlessFlow {
                    session_id,
                    tag: tag_owned,
                })
            }
            #[cfg(feature = "hysteria2")]
            ResolvedLeafOutbound::Hysteria2 {
                tag,
                server,
                port,
                password,
                client_fingerprint,
                ..
            } => {
                let sent = self
                    .h2_manager
                    .send(
                        &mut self.chain_tasks,
                        session.id,
                        proxy,
                        server,
                        port,
                        password,
                        client_fingerprint,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await
                    .map_err(|f: FlowFailure| FlowFailure {
                        stage: f.stage,
                        error: f.error,
                        upstream: f.upstream,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Hysteria2 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "hysteria2"))]
            ResolvedLeafOutbound::Hysteria2 { .. } => Err(FlowFailure {
                stage: "udp_hysteria2_outbound",
                error: zero_core::Error::Unsupported(
                    "Hysteria2 UDP outbound requires Cargo feature `hysteria2`",
                )
                .into(),
                upstream: None,
            }),
            #[allow(unused_variables)]
            ResolvedLeafOutbound::Shadowsocks {
                tag,
                server,
                port,
                password,
                cipher,
                ..
            } => {
                #[cfg(feature = "shadowsocks")]
                {
                    let sent = self
                        .ss_manager
                        .send(
                            &mut self.chain_tasks,
                            session.id,
                            server,
                            port,
                            password,
                            cipher,
                            &session.target,
                            session.port,
                            payload,
                        )
                        .await
                        .map_err(|f: FlowFailure| FlowFailure {
                            stage: f.stage,
                            error: f.error,
                            upstream: f.upstream,
                        })?;

                    Ok(FlowStartResult::Flow {
                        outbound: UdpFlowOutbound::Shadowsocks {
                            tag: tag.to_owned(),
                            server: server.to_owned(),
                            port,
                            password: password.to_owned(),
                            cipher: cipher.to_owned(),
                            packet_path_carrier: None,
                        },
                        tx_bytes: sent as u64,
                    })
                }
                #[cfg(not(feature = "shadowsocks"))]
                {
                    Err(FlowFailure {
                        stage: "udp_shadowsocks_outbound",
                        error: zero_core::Error::Unsupported(
                            "Shadowsocks UDP outbound requires Cargo feature `shadowsocks`",
                        )
                        .into(),
                        upstream: None,
                    })
                }
            }
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Trojan {
                tag,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
            } => {
                let sent = self
                    .trojan_manager
                    .send(
                        &mut self.chain_tasks,
                        session.id,
                        proxy,
                        session,
                        server,
                        port,
                        password,
                        sni,
                        insecure,
                        client_fingerprint,
                        false,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await
                    .map_err(|f: FlowFailure| FlowFailure {
                        stage: f.stage,
                        error: f.error,
                        upstream: f.upstream,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Trojan {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        sni: sni.map(|s| s.to_owned()),
                        insecure,
                        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
                        relay_chain: false,
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "trojan"))]
            ResolvedLeafOutbound::Trojan { .. } => Err(FlowFailure {
                stage: "udp_trojan_outbound",
                error: zero_core::Error::Unsupported(
                    "Trojan UDP outbound requires Cargo feature `trojan`",
                )
                .into(),
                upstream: None,
            }),
            #[cfg(feature = "mieru")]
            ResolvedLeafOutbound::Mieru {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let sent = self
                    .mieru_manager
                    .send(
                        &mut self.chain_tasks,
                        session.id,
                        proxy,
                        session,
                        server,
                        port,
                        username,
                        password,
                        false,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await
                    .map_err(|f: FlowFailure| FlowFailure {
                        stage: f.stage,
                        error: f.error,
                        upstream: f.upstream,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Mieru {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.to_owned(),
                        password: password.to_owned(),
                        relay_chain: false,
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "mieru"))]
            ResolvedLeafOutbound::Mieru { .. } => Err(FlowFailure {
                stage: "udp_mieru_outbound",
                error: zero_core::Error::Unsupported(
                    "Mieru UDP outbound requires Cargo feature `mieru`",
                )
                .into(),
                upstream: None,
            }),
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Vmess { .. } => Err(FlowFailure {
                stage: "vmess",
                error: zero_core::Error::Unsupported("vmess UDP not supported").into(),
                upstream: None,
            }),
            #[cfg(not(feature = "trojan"))]
            ResolvedLeafOutbound::Vmess { .. } => Err(FlowFailure {
                stage: "vmess",
                error: zero_core::Error::Unsupported("vmess UDP not supported").into(),
                upstream: None,
            }),
        }
    }

    async fn start_relay_flow(
        &mut self,
        proxy: &Proxy,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        // Datagram-over-packet-path: previous hop provides a packet path,
        // next hop encodes its datagram through it.
        #[cfg(all(feature = "socks5", feature = "shadowsocks"))]
        if let Some(params) = resolve_udp_packet_path_chain(&chain) {
            let sent = self
                .packet_path_manager
                .send(
                    &mut self.chain_tasks,
                    session.id,
                    proxy,
                    &params,
                    &session.target,
                    session.port,
                    payload,
                )
                .await?;

            return Ok(FlowStartResult::Flow {
                outbound: UdpFlowOutbound::Shadowsocks {
                    tag: params.datagram_tag.to_owned(),
                    server: params.datagram_server.to_owned(),
                    port: params.datagram_port,
                    password: params.datagram_password.to_owned(),
                    cipher: params.datagram_cipher.to_owned(),
                    packet_path_carrier: Some(UdpPacketPathCarrier {
                        tag: params.carrier_tag.to_owned(),
                        server: params.carrier_server.to_owned(),
                        port: params.carrier_port,
                        username: params.carrier_username.map(ToOwned::to_owned),
                        password: params.carrier_password.map(ToOwned::to_owned),
                    }),
                },
                tx_bytes: sent as u64,
            });
        }

        let (stream, final_hop) = proxy
            .establish_relay_prefix(chain)
            .await
            .map_err(|failure| FlowFailure {
                stage: failure.stage,
                error: failure.error,
                upstream: failure.upstream_endpoint,
            })?;

        match final_hop {
            ResolvedLeafOutbound::Vless {
                tag,
                server,
                port,
                id,
                tls,
                reality,
                ws,
                grpc,
                h2,
                http_upgrade,
                split_http,
                quic,
                ..
            } => {
                if quic.is_some() {
                    return Err(FlowFailure {
                        stage: "udp_relay_final_transport",
                        error: zero_core::Error::Unsupported(
                            "VLESS QUIC final hop over TCP relay chain is not supported",
                        )
                        .into(),
                        upstream: None,
                    });
                }

                let session_id = session.id;
                let tag_owned = tag.to_owned();
                let key = (session.target.clone(), session.port);
                let stream = crate::transport::build_vless_outbound_transport_over_stream(
                    stream,
                    tls,
                    reality,
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    proxy.config.source_dir(),
                    server,
                    port,
                )
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_relay_final_transport",
                    error,
                    upstream: Some((server.to_owned(), port)),
                })?;
                let (upstream, recv_tx) =
                    establish_vless_udp_upstream_over_stream(proxy, session, id, payload, stream)
                        .await
                        .map_err(|error| FlowFailure {
                            stage: "udp_vless_relay_chain",
                            error,
                            upstream: None,
                        })?;
                self.vless_manager.insert_upstream(key, upstream, recv_tx);
                self.vless_manager.spawn_bridge(
                    &mut self.chain_tasks,
                    session.target.clone(),
                    session.port,
                    session_id,
                );

                Ok(FlowStartResult::VlessFlow {
                    session_id,
                    tag: tag_owned,
                })
            }
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Trojan {
                tag,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
            } => {
                let sent = self
                    .trojan_manager
                    .send_relay(
                        &mut self.chain_tasks,
                        session.id,
                        stream,
                        None,
                        proxy,
                        session,
                        server,
                        port,
                        password,
                        sni,
                        insecure,
                        client_fingerprint,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Trojan {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        sni: sni.map(|s| s.to_owned()),
                        insecure,
                        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
                        relay_chain: true,
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(feature = "mieru")]
            ResolvedLeafOutbound::Mieru {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let sent = self
                    .mieru_manager
                    .send_relay(
                        &mut self.chain_tasks,
                        session.id,
                        stream,
                        server,
                        port,
                        username,
                        password,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Mieru {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.to_owned(),
                        password: password.to_owned(),
                        relay_chain: true,
                    },
                    tx_bytes: sent as u64,
                })
            }
            _ => Err(FlowFailure {
                stage: "udp_relay_final_hop",
                error: zero_core::Error::Unsupported(
                    "UDP relay chain final hop does not support stream packet UDP",
                )
                .into(),
                upstream: None,
            }),
        }
    }
}
