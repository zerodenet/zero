use std::time::Instant;

use zero_core::{Address, Network, ProtocolType, Session};
use zero_engine::{EngineError, ResolvedOutbound, SessionOutcome};
use socks5::parse_udp_packet;
use zero_traits::DnsResolver;

use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
use crate::runtime::Proxy;
use crate::transport::StreamTraffic;

use super::context::{
    ExistingUdpFlowContext, Socks5UdpPacketContext, UdpCandidateContext, UdpCandidateStart,
    UdpRequestContext,
};
use super::sessions::{UdpFlowOutbound, UdpFlowSnapshot};

impl Proxy {
    pub(super) async fn handle_socks5_udp_request(
        &self,
        packet: &[u8],
        context: UdpRequestContext<'_>,
    ) -> Result<(), EngineError> {
        let udp_packet = parse_udp_packet(packet)?;

        // ── DNS interception ─────────────────────────────────────────
        // Intercept UDP packets to port 53 with a domain target.
        // Resolve locally through DnsSystem and reply directly.
        if udp_packet.port == 53 {
            if let Address::Domain(ref domain) = udp_packet.target {
                if let Some(client_addr) = context.client_addr {
                    match self.resolver.resolve(domain).await {
                        Ok(ips) => {
                            let dns_resp =
                                zero_dns::udp::build_dns_response(&udp_packet.payload, &ips);
                            if !dns_resp.is_empty() {
                                let frame = socks5::build_udp_packet(
                                    &udp_packet.target,
                                    udp_packet.port,
                                    &dns_resp,
                                )?;
                                let _ = context.relay.send_to_addr(&frame, client_addr).await;
                            }
                        }
                        Err(_) => {
                            // Resolution failed — silently drop.
                            // The client's stub resolver will retry.
                        }
                    }
                    return Ok(());
                }
            }
        }

        if let Some(flow) = context
            .udp_flows
            .snapshot(&udp_packet.target, udp_packet.port)
        {
            self.forward_existing_udp_flow(
                &flow,
                packet.len() as u64,
                &udp_packet.payload,
                ExistingUdpFlowContext {
                    inbound_tag: context.inbound_tag,
                    relay: context.relay,
                    udp_flows: context.udp_flows,
                    upstream_association: context.upstream_association,
                    upstream_idle_deadline: context.upstream_idle_deadline,
                },
            )
            .await?;
            return Ok(());
        }

        let mut session = Session::new(
            0,
            udp_packet.target,
            udp_packet.port,
            Network::Udp,
            ProtocolType::Socks5,
        );
        self.prepare_session(&mut session, context.inbound_tag, None);
        let mut session_handle = self.track_session(session.id);
        let started_at = Instant::now();
        self.record_session_inbound_traffic(session.id, *context.pending_control_traffic);
        *context.pending_control_traffic = StreamTraffic::default();
        self.record_session_inbound_rx(session.id, packet.len() as u64);

        self.resolve_fake_ip_target(&mut session).await;
        let action = self.route_decision(&session);
        let resolved = match self.resolve_outbound(&action) {
            Ok((resolved, _plan)) => resolved,
            Err(error) => {
                let record = session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &session,
                    record.as_ref(),
                    "resolve_outbound",
                    started_at.elapsed(),
                    &error,
                    None,
                );
                return Err(error);
            }
        };
        log_session_accepted(&session, &action, self.config.mode.kind());

        let candidates = match resolved {
            ResolvedOutbound::Single(candidate) => vec![candidate],
            ResolvedOutbound::Fallback { candidates } => candidates,
            ResolvedOutbound::Relay { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "relay chain not supported for UDP flows",
                )))
            }
        };
        let is_fallback = candidates.len() > 1;
        let mut last_failure = None;

        for candidate in candidates {
            match self
                .start_udp_flow_candidate(
                    candidate,
                    UdpCandidateContext {
                        inbound_tag: context.inbound_tag,
                        relay: context.relay,
                        session: &session,
                        payload: &udp_packet.payload,
                        upstream_association: context.upstream_association,
                        upstream_idle_deadline: context.upstream_idle_deadline,
                    },
                )
                .await
            {
                Ok(UdpCandidateStart::Flow {
                    outbound,
                    outbound_tx_bytes,
                }) => {
                    let session_id = session.id;
                    session.outbound_tag = Some(outbound.tag().to_owned());
                    self.set_session_outbound(&session);
                    context.udp_flows.insert(session, session_handle, outbound);
                    self.record_session_outbound_tx(session_id, outbound_tx_bytes);
                    return Ok(());
                }
                Ok(UdpCandidateStart::Blocked { tag }) => {
                    session.outbound_tag = Some(tag);
                    self.set_session_outbound(&session);
                    if let Some(record) = session_handle.finish(SessionOutcome::Blocked) {
                        log_session_finished(&record, None);
                    }
                    return Ok(());
                }
                Err(failure) => {
                    last_failure = Some(failure);
                }
            }
        }

        let record = session_handle.finish(SessionOutcome::Failed);
        if let Some(failure) = last_failure {
            let stage = if is_fallback {
                "fallback_exhausted"
            } else {
                failure.stage
            };
            log_session_failed(
                &session,
                record.as_ref(),
                stage,
                started_at.elapsed(),
                &failure.error,
                failure
                    .upstream
                    .as_ref()
                    .map(|(server, port): &(String, u16)| (server.as_str(), *port)),
            );
        } else {
            let error = EngineError::Io(std::io::Error::other("all fallback outbounds failed"));
            log_session_failed(
                &session,
                record.as_ref(),
                "fallback_exhausted",
                started_at.elapsed(),
                &error,
                None,
            );
        }

        Ok(())
    }

    async fn forward_existing_udp_flow(
        &self,
        flow: &UdpFlowSnapshot,
        inbound_rx_bytes: u64,
        payload: &[u8],
        context: ExistingUdpFlowContext<'_>,
    ) -> Result<(), EngineError> {
        let started_at = Instant::now();
        self.record_session_inbound_rx(flow.session.id, inbound_rx_bytes);

        match &flow.outbound {
            UdpFlowOutbound::Direct { target_addr, .. } => {
                match context.relay.send_to_addr(payload, *target_addr).await {
                    Ok(sent) => {
                        self.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        if let Some(completed) = context.udp_flows.finish(
                            &flow.session.target,
                            flow.session.port,
                            SessionOutcome::Failed,
                        ) {
                            log_session_failed(
                                &flow.session,
                                Some(&completed.record),
                                "udp_direct_send",
                                started_at.elapsed(),
                                &error,
                                None,
                            );
                        } else {
                            log_session_failed(
                                &flow.session,
                                None,
                                "udp_direct_send",
                                started_at.elapsed(),
                                &error,
                                None,
                            );
                        }
                        return Err(error.into());
                    }
                }
            }
            UdpFlowOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => {
                match self
                    .send_socks5_udp_packet(Socks5UdpPacketContext {
                        inbound_tag: context.inbound_tag,
                        tag,
                        server,
                        port: *port,
                        auth: username.as_deref().zip(password.as_deref()),
                        session: &flow.session,
                        payload,
                        upstream_association: context.upstream_association,
                        upstream_idle_deadline: context.upstream_idle_deadline,
                    })
                    .await
                {
                    Ok(sent) => {
                        self.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        let stage = failure.stage;
                        let upstream = failure.upstream;
                        let error = failure.error;
                        if let Some(completed) = context.udp_flows.finish(
                            &flow.session.target,
                            flow.session.port,
                            SessionOutcome::Failed,
                        ) {
                            log_session_failed(
                                &flow.session,
                                Some(&completed.record),
                                stage,
                                started_at.elapsed(),
                                &error,
                                upstream
                                    .as_ref()
                                    .map(|(s, p): &(String, u16)| (s.as_str(), *p)),
                            );
                        } else {
                            log_session_failed(
                                &flow.session,
                                None,
                                stage,
                                started_at.elapsed(),
                                &error,
                                upstream
                                    .as_ref()
                                    .map(|(s, p): &(String, u16)| (s.as_str(), *p)),
                            );
                        }
                        return Err(error);
                    }
                }
            }
            #[cfg(feature = "shadowsocks")]
            UdpFlowOutbound::Shadowsocks {
                tag: _,
                server,
                port,
                password,
                cipher,
            } => {
                use crate::outbound::shadowsocks::send_ss_udp_packet;
                match send_ss_udp_packet(
                    server.as_str(),
                    *port,
                    password.as_str(),
                    cipher.as_str(),
                    &flow.session.target,
                    flow.session.port,
                    payload,
                )
                .await
                {
                    Ok(sent) => {
                        self.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        let msg = error.to_string();
                        if let Some(completed) = context.udp_flows.finish(
                            &flow.session.target,
                            flow.session.port,
                            SessionOutcome::Failed,
                        ) {
                            log_session_failed(
                                &flow.session,
                                Some(&completed.record),
                                "udp_ss_send",
                                started_at.elapsed(),
                                &EngineError::Io(std::io::Error::other(msg.as_str())),
                                None,
                            );
                        }
                        return Err(EngineError::Io(std::io::Error::other(msg.as_str())));
                    }
                }
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
                use crate::outbound::hysteria2::send_h2_udp_packet;
                match send_h2_udp_packet(
                    self,
                    &flow.session,
                    server.as_str(),
                    *port,
                    password.as_str(),
                    client_fingerprint.as_deref(),
                    &flow.session.target,
                    flow.session.port,
                    payload,
                )
                .await
                {
                    Ok(sent) => {
                        self.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        let msg = error.to_string();
                        if let Some(completed) = context.udp_flows.finish(
                            &flow.session.target,
                            flow.session.port,
                            SessionOutcome::Failed,
                        ) {
                            log_session_failed(
                                &flow.session,
                                Some(&completed.record),
                                "udp_h2_send",
                                started_at.elapsed(),
                                &EngineError::Io(std::io::Error::other(msg.as_str())),
                                None,
                            );
                        }
                        return Err(EngineError::Io(std::io::Error::other(msg.as_str())));
                    }
                }
            }
            #[cfg(not(feature = "hysteria2"))]
            UdpFlowOutbound::Hysteria2 { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Hysteria2 UDP outbound requires feature `hysteria2`",
                )));
            }
            #[cfg(feature = "trojan")]
            UdpFlowOutbound::Trojan {
                tag: _,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
            } => {
                use crate::outbound::trojan::send_trojan_udp_packet;
                match send_trojan_udp_packet(
                    self,
                    &flow.session,
                    server.as_str(),
                    *port,
                    password.as_str(),
                    sni.as_deref(),
                    *insecure,
                    client_fingerprint.as_deref(),
                    &flow.session.target,
                    flow.session.port,
                    payload,
                )
                .await
                {
                    Ok(sent) => {
                        self.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        let msg = error.to_string();
                        if let Some(completed) = context.udp_flows.finish(
                            &flow.session.target,
                            flow.session.port,
                            SessionOutcome::Failed,
                        ) {
                            log_session_failed(
                                &flow.session,
                                Some(&completed.record),
                                "udp_trojan_send",
                                started_at.elapsed(),
                                &EngineError::Io(std::io::Error::other(msg.as_str())),
                                None,
                            );
                        }
                        return Err(EngineError::Io(std::io::Error::other(msg.as_str())));
                    }
                }
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
            } => {
                use crate::outbound::mieru_udp::send_mieru_udp_packet;
                match send_mieru_udp_packet(
                    self,
                    &flow.session,
                    server.as_str(),
                    *port,
                    username.as_str(),
                    password.as_str(),
                    &flow.session.target,
                    flow.session.port,
                    payload,
                )
                .await
                {
                    Ok(sent) => {
                        self.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        let msg = error.to_string();
                        if let Some(completed) = context.udp_flows.finish(
                            &flow.session.target,
                            flow.session.port,
                            SessionOutcome::Failed,
                        ) {
                            log_session_failed(
                                &flow.session,
                                Some(&completed.record),
                                "udp_mieru_send",
                                started_at.elapsed(),
                                &EngineError::Io(std::io::Error::other(msg.as_str())),
                                None,
                            );
                        }
                        return Err(EngineError::Io(std::io::Error::other(msg.as_str())));
                    }
                }
            }
            #[cfg(not(feature = "mieru"))]
            UdpFlowOutbound::Mieru { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Mieru UDP outbound requires feature `mieru`",
                )));
            }
        }

        Ok(())
    }
}
