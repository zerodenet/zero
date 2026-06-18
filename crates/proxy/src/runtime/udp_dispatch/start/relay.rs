use super::*;

impl UdpDispatch {
    pub(super) async fn start_relay_flow(
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

        // ── XHTTP two-stream fast path (packet-up / stream-up only) ─────
        // The legacy two-connection XHTTP model uses separate POST (write) and
        // GET (read) channels. Run the relay prefix setup twice so we get two
        // independent TCP streams through the same set of intermediate hops.
        //
        // `stream-one` / `auto` use a single bidirectional connection and fall
        // through to the generic single-stream final-hop path below.
        let needs_two_streams = matches!(
            chain.last(),
            Some(ResolvedLeafOutbound::Vless {
                split_http: Some(cfg),
                ..
            }) if !zero_transport::split_http::XhttpMode::parse(&cfg.mode).is_single_connection()
        );
        if needs_two_streams {
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
