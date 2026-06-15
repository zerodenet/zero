//! UDP flow start: new outbound establishment.
//!
//! Contains [`UdpDispatch::start_flow`] (single-hop) and
//! [`UdpDispatch::start_relay_flow`] (multi-hop chain) for establishing new
//! UDP outbound connections, plus the chain resolution function
//! [`resolve_udp_packet_path_chain`].

use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;
use zero_traits::UdpPacketFraming;

#[cfg(feature = "shadowsocks")]
use super::packet_path_chain::{PacketPathCarrierParams, PacketPathChainParams};
use super::{
    FlowFailure, FlowStartResult, H2UdpPeer, MieruUdpPeer, SsUdpPeer, TrojanUdpPeer, UdpCandidate,
    UdpDispatch, UdpFlowContext, UdpPacketRef, UdpPeerEndpoint,
};
use crate::runtime::udp_associate::sessions::{UdpFlowOutbound, UdpPacketPathCarrier};
use crate::runtime::vless_udp::{establish_vless_udp_upstream_over_stream, VlessUdpTransport};
#[cfg(feature = "vmess")]
use crate::runtime::vmess_udp::{
    build_vmess_udp_transport_over_stream, establish_vmess_udp_upstream_over_stream,
    VmessUdpTransport,
};
use crate::runtime::Proxy;

// Chain resolution.

/// Resolve a relay chain into packet-path + datagram parameters.
///
/// Returns `Some` when the chain matches the "packet path carrier -> datagram
/// protocol" pattern. Currently recognises `[Shadowsocks, Shadowsocks]` and,
/// when the `socks5` feature is enabled, `[SOCKS5, Shadowsocks]`. Adding
/// new combinations only requires extending this function and implementing
/// [`UdpPacketPath`] + [`DatagramCodec`]: no new protocol-pair modules.
#[cfg(feature = "shadowsocks")]
fn resolve_udp_packet_path_chain<'a>(
    chain: &[ResolvedLeafOutbound<'a>],
) -> Option<PacketPathChainParams<'a>> {
    match chain {
        #[cfg(feature = "socks5")]
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
            carrier: PacketPathCarrierParams::Socks5 {
                tag: carrier_tag,
                server: carrier_server,
                port: *carrier_port,
                username: *carrier_username,
                password: *carrier_password,
            },
            datagram_server,
            datagram_port: *datagram_port,
            datagram_password,
            datagram_cipher,
        }),
        [ResolvedLeafOutbound::Shadowsocks {
            tag: carrier_tag,
            server: carrier_server,
            port: carrier_port,
            password: carrier_password,
            cipher: carrier_cipher,
        }, ResolvedLeafOutbound::Shadowsocks {
            tag: datagram_tag,
            server: datagram_server,
            port: datagram_port,
            password: datagram_password,
            cipher: datagram_cipher,
        }] => Some(PacketPathChainParams {
            datagram_tag,
            carrier: PacketPathCarrierParams::Shadowsocks {
                tag: carrier_tag,
                server: carrier_server,
                port: *carrier_port,
                password: carrier_password,
                cipher: carrier_cipher,
            },
            datagram_server,
            datagram_port: *datagram_port,
            datagram_password,
            datagram_cipher,
        }),
        _ => None,
    }
}

#[cfg(feature = "shadowsocks")]
fn owned_packet_path_carrier(carrier: &PacketPathCarrierParams<'_>) -> UdpPacketPathCarrier {
    match carrier {
        #[cfg(feature = "socks5")]
        PacketPathCarrierParams::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } => UdpPacketPathCarrier::Socks5 {
            tag: (*tag).to_owned(),
            server: (*server).to_owned(),
            port: *port,
            username: username.map(ToOwned::to_owned),
            password: password.map(ToOwned::to_owned),
        },
        PacketPathCarrierParams::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } => UdpPacketPathCarrier::Shadowsocks {
            tag: (*tag).to_owned(),
            server: (*server).to_owned(),
            port: *port,
            password: (*password).to_owned(),
            cipher: (*cipher).to_owned(),
        },
    }
}

// impl UdpDispatch.

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
                flow,
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
                let session_id = session.id;
                let tag_owned = tag.to_owned();

                // MUX UDP fast path: when Vision flow is active and a MUX
                // pool connection already exists, open a UDP sub-stream
                // through the shared MUX connection instead of dialing a
                // fresh VLESS upstream.
                let mux_flow_enabled =
                    flow == Some("xtls-rprx-vision") || flow == Some("xtls-rprx-vision-udp443");
                if mux_flow_enabled {
                    // Try to open a MUX UDP sub-stream from the pool.
                    // If the pool has no connection to this upstream yet
                    // (first packet), fall through to the normal VLESS
                    // manager path which dials a fresh connection.
                    let max_concurrency = 8u32; // config may override later
                    let idle_timeout = 300u64;
                    match proxy.mux_pool.open_udp_stream(
                        proxy,
                        server,
                        port,
                        &vless::parse_uuid(id).map_err(|e| FlowFailure {
                            stage: "udp_vless_mux_parse_uuid",
                            error: zero_core::Error::Protocol(
                                &*Box::leak(
                                    format!("invalid VLESS UUID: {e}").into_boxed_str(),
                                ),
                            )
                            .into(),
                            upstream: Some((server.to_owned(), port)),
                        })?,
                        tls,
                        reality,
                        max_concurrency,
                        idle_timeout,
                    ).await {
                        Ok((_mux_sid, up_tx, _down_rx)) => {
                            // Encode the first VLESS UDP packet and send it
                            // through the MUX UDP sub-stream.
                            let packet = <vless::VlessOutbound as UdpPacketFraming<
                                vless::VlessUdpPacketTarget,
                            >>::encode_udp_packet(
                                &proxy.protocols.vless_outbound,
                                &vless::VlessUdpPacketTarget {
                                    address: &session.target,
                                    port: session.port,
                                    payload,
                                },
                            ).map_err(|error| FlowFailure {
                                stage: "udp_vless_mux_encode",
                                error: error.into(),
                                upstream: Some((server.to_owned(), port)),
                            })?;
                            let _ = up_tx.send(packet);
                            // MUX UDP responses are dispatched back through
                            // the MUX read loop → chain_tasks; the session
                            // handle is tracked in vless_handles.
                            proxy.record_session_outbound_tx(
                                session_id,
                                payload.len() as u64,
                            );

                            // Track this session in vless_handles so finish_all()
                            // properly completes it. The MUX read loop dispatches
                            // responses back through chain_tasks.
                            return Ok(FlowStartResult::VlessFlow {
                                session_id,
                                tag: tag_owned,
                            });
                        }
                        // MUX pool has no connection yet, fall through.
                        Err(_) => {}
                    }
                }

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
                        UdpFlowContext {
                            chain_tasks: &mut self.chain_tasks,
                            session_id: session.id,
                        },
                        H2UdpPeer {
                            endpoint: UdpPeerEndpoint { server, port },
                            password,
                            client_fingerprint,
                        },
                        UdpPacketRef {
                            target: &session.target,
                            port: session.port,
                            payload,
                        },
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
                            UdpFlowContext {
                                chain_tasks: &mut self.chain_tasks,
                                session_id: session.id,
                            },
                            proxy,
                            SsUdpPeer {
                                endpoint: UdpPeerEndpoint { server, port },
                                password,
                                cipher,
                            },
                            UdpPacketRef {
                                target: &session.target,
                                port: session.port,
                                payload,
                            },
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
                        UdpFlowContext {
                            chain_tasks: &mut self.chain_tasks,
                            session_id: session.id,
                        },
                        proxy,
                        session,
                        TrojanUdpPeer {
                            endpoint: UdpPeerEndpoint { server, port },
                            password,
                            sni,
                            insecure,
                            client_fingerprint,
                            relay_chain: false,
                        },
                        UdpPacketRef {
                            target: &session.target,
                            port: session.port,
                            payload,
                        },
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
                        UdpFlowContext {
                            chain_tasks: &mut self.chain_tasks,
                            session_id: session.id,
                        },
                        proxy,
                        session,
                        MieruUdpPeer {
                            endpoint: UdpPeerEndpoint { server, port },
                            username,
                            password,
                            relay_chain: false,
                        },
                        UdpPacketRef {
                            target: &session.target,
                            port: session.port,
                            payload,
                        },
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
            #[cfg(feature = "vmess")]
            ResolvedLeafOutbound::Vmess {
                tag,
                server,
                port,
                id,
                cipher,
                mux_concurrency,
                mux_idle_timeout_secs: _,
                tls,
                ws,
                grpc,
            } => {
                let transport = VmessUdpTransport { tls, ws, grpc };
                let session_id = session.id;
                let tag_owned = tag.to_owned();
                self.vmess_manager
                    .get_or_create_upstream(
                        &mut self.chain_tasks,
                        proxy,
                        session,
                        session.target.clone(),
                        session.port,
                        server.to_owned(),
                        port,
                        id.to_owned(),
                        cipher.to_owned(),
                        payload.to_vec(),
                        Some(&transport),
                        mux_concurrency,
                    )
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_vmess_upstream",
                        error,
                        upstream: Some((server.to_owned(), port)),
                    })?;

                Ok(FlowStartResult::VmessFlow {
                    session_id,
                    tag: tag_owned,
                })
            }
            #[cfg(not(feature = "vmess"))]
            ResolvedLeafOutbound::Vmess { .. } => Err(FlowFailure {
                stage: "udp_vmess_outbound",
                error: zero_core::Error::Unsupported(
                    "VMess UDP outbound requires Cargo feature `vmess`",
                )
                .into(),
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
        #[cfg(feature = "shadowsocks")]
        if let Some(params) = resolve_udp_packet_path_chain(&chain) {
            let sent = self
                .packet_path_manager
                .send(
                    UdpFlowContext {
                        chain_tasks: &mut self.chain_tasks,
                        session_id: session.id,
                    },
                    proxy,
                    &params,
                    UdpPacketRef {
                        target: &session.target,
                        port: session.port,
                        payload,
                    },
                )
                .await?;

            return Ok(FlowStartResult::Flow {
                outbound: UdpFlowOutbound::Shadowsocks {
                    tag: params.datagram_tag.to_owned(),
                    server: params.datagram_server.to_owned(),
                    port: params.datagram_port,
                    password: params.datagram_password.to_owned(),
                    cipher: params.datagram_cipher.to_owned(),
                    packet_path_carrier: Some(owned_packet_path_carrier(&params.carrier)),
                },
                tx_bytes: sent as u64,
            });
        }

        // ── SplitHTTP fast path (needs two relay prefix streams) ───────
        // SplitHTTP uses separate POST (write) and GET (read) channels.
        // Run the relay prefix setup twice so we get two independent TCP
        // streams through the same set of intermediate hops.
        if matches!(
            chain.last(),
            Some(ResolvedLeafOutbound::Vless {
                split_http: Some(_),
                ..
            })
        ) {
            let chain_get = chain.clone();
            let (post_carrier, final_hop) =
                proxy
                    .dispatch_tcp_relay_prefix(chain)
                    .await
                    .map_err(|failure| FlowFailure {
                        stage: failure.stage,
                        error: failure.error,
                        upstream: failure.upstream_endpoint,
                    })?;
            let (get_carrier, _) =
                proxy
                    .dispatch_tcp_relay_prefix(chain_get)
                    .await
                    .map_err(|failure| FlowFailure {
                        stage: failure.stage,
                        error: failure.error,
                        upstream: failure.upstream_endpoint,
                    })?;

            let (tag, server, port, id, split_http_cfg) = match &final_hop {
                ResolvedLeafOutbound::Vless {
                    tag,
                    server,
                    port,
                    id,
                    split_http,
                    ..
                } => (
                    tag,
                    server,
                    port,
                    id,
                    split_http.as_ref().expect("checked above"),
                ),
                _ => unreachable!(),
            };

            let stream = crate::transport::build_vless_split_http_over_relay(
                post_carrier.stream,
                get_carrier.stream,
                split_http_cfg,
            )
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_relay_final_transport",
                error,
                upstream: Some((server.to_string(), *port)),
            })?;

            let session_id = session.id;
            let key = (session.target.clone(), session.port);
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

            return Ok(FlowStartResult::VlessFlow {
                session_id,
                tag: tag.to_string(),
            });
        }

        let (carrier, final_hop) =
            proxy
                .dispatch_tcp_relay_prefix(chain)
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
                    carrier,
                    tls,
                    reality,
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    proxy.config.source_dir(),
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
                        UdpFlowContext {
                            chain_tasks: &mut self.chain_tasks,
                            session_id: session.id,
                        },
                        carrier.stream,
                        None,
                        proxy,
                        session,
                        TrojanUdpPeer {
                            endpoint: UdpPeerEndpoint { server, port },
                            password,
                            sni,
                            insecure,
                            client_fingerprint,
                            relay_chain: true,
                        },
                        UdpPacketRef {
                            target: &session.target,
                            port: session.port,
                            payload,
                        },
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
                        UdpFlowContext {
                            chain_tasks: &mut self.chain_tasks,
                            session_id: session.id,
                        },
                        carrier.stream,
                        MieruUdpPeer {
                            endpoint: UdpPeerEndpoint { server, port },
                            username,
                            password,
                            relay_chain: true,
                        },
                        UdpPacketRef {
                            target: &session.target,
                            port: session.port,
                            payload,
                        },
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
            #[cfg(feature = "vmess")]
            ResolvedLeafOutbound::Vmess {
                tag,
                server,
                port,
                id,
                cipher,
                mux_concurrency: _,
                mux_idle_timeout_secs: _,
                tls,
                ws,
                grpc,
            } => {
                let session_id = session.id;
                let tag_owned = tag.to_owned();
                let key = (session.target.clone(), session.port);
                let transport = VmessUdpTransport { tls, ws, grpc };
                let stream = build_vmess_udp_transport_over_stream(
                    carrier.stream,
                    Some(&transport),
                    proxy.config.source_dir(),
                    server,
                    port,
                )
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_vmess_relay_final_transport",
                    error,
                    upstream: Some((server.to_owned(), port)),
                })?;
                let (upstream, recv_tx) = establish_vmess_udp_upstream_over_stream(
                    proxy, session, id, cipher, payload, stream,
                )
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_vmess_relay_chain",
                    error,
                    upstream: None,
                })?;
                self.vmess_manager.insert_upstream(key, upstream, recv_tx);
                self.vmess_manager.spawn_bridge(
                    &mut self.chain_tasks,
                    session.target.clone(),
                    session.port,
                    session_id,
                );

                Ok(FlowStartResult::VmessFlow {
                    session_id,
                    tag: tag_owned,
                })
            }
            _ => Err(FlowFailure {
                stage: "udp_relay_final_hop",
                error: zero_core::Error::Unsupported(
                    "UDP relay chain final hop not supported (supported: VLESS, Trojan, Mieru, VMess)",
                )
                .into(),
                upstream: None,
            }),
        }
    }
}
