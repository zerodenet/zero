use std::net::{IpAddr, SocketAddr};
use std::time::Instant;

use tokio::select;
use tokio::time::{sleep_until, Instant as TokioInstant};
use tracing::{debug, info, warn};
use zero_core::{Address, Network, ProtocolType, Session};
use zero_platform_tokio::TokioDatagramSocket;
use zero_protocol_socks5::{
    build_udp_packet, parse_udp_packet, Socks5Reply, Socks5UdpAssociateRequest,
};
use zero_traits::AsyncSocket;

use super::error::EngineError;
use super::logging::{
    log_session_accepted, log_session_failed, log_session_finished,
    log_udp_upstream_association_created, log_udp_upstream_association_dropped,
    log_udp_upstream_association_idle_timeout, log_udp_upstream_association_reused,
};
use super::metered::{MeteredStream, StreamTraffic};
use super::resolve::{ResolvedLeafOutbound, ResolvedOutbound};
use super::runtime::Engine;
use super::stats::SessionOutcome;
use super::stream::ClientStream;
use super::udp_sessions::{CompletedUdpFlow, UdpFlowOutbound, UdpFlowSnapshot, UdpSessionFlows};
use super::upstream_socks5_udp::{
    ActiveUpstreamSocks5UdpAssociation, UpstreamAssociationCloseReason,
};

impl Engine {
    pub(crate) async fn handle_socks5_udp_associate<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        _request: Socks5UdpAssociateRequest,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        let control_local_addr = client.local_addr()?;
        let relay =
            TokioDatagramSocket::bind_addr(SocketAddr::new(control_local_addr.ip(), 0)).await?;
        let relay_addr = relay.local_addr()?;
        let relay_bind = address_from_socket_addr(relay_addr);

        self.protocols
            .socks5_inbound
            .send_response_with_bound(
                &mut client,
                Socks5Reply::Succeeded,
                &relay_bind,
                relay_addr.port(),
            )
            .await?;
        let mut pending_control_traffic = client.drain_traffic();

        info!(
            inbound_tag = inbound_tag,
            protocol = "socks5-udp",
            relay = %relay_addr,
            "socks5 udp association ready"
        );

        let mut client_udp_addr: Option<SocketAddr> = None;
        let mut upstream_association: Option<ActiveUpstreamSocks5UdpAssociation> = None;
        let mut upstream_idle_deadline: Option<TokioInstant> = None;
        let mut udp_flows = UdpSessionFlows::default();
        let mut control_probe = [0_u8; 1];
        let mut packet = vec![0_u8; 64 * 1024];
        let mut upstream_packet = vec![0_u8; 64 * 1024];

        loop {
            select! {
                control = client.read(&mut control_probe) => {
                    match control {
                        Ok(0) => break,
                        Ok(_) => break,
                        Err(error) => return Err(error.into()),
                    }
                }
                recv = relay.recv_from_addr(&mut packet) => {
                    let (read, sender) = recv?;
                    let buf = &packet[..read];

                    if client_udp_addr.is_none() {
                        client_udp_addr = Some(sender);
                    }

                    if client_udp_addr == Some(sender) {
                        if let Err(error) = self.handle_socks5_udp_request(
                            buf,
                            UdpRequestContext {
                                inbound_tag,
                                relay: &relay,
                                udp_flows: &mut udp_flows,
                                pending_control_traffic: &mut pending_control_traffic,
                                upstream_association: &mut upstream_association,
                                upstream_idle_deadline: &mut upstream_idle_deadline,
                            },
                        )
                        .await {
                            warn!(
                                inbound_tag = inbound_tag,
                                protocol = "socks5-udp",
                                error = %error,
                                "failed to process UDP packet"
                            );
                        }
                    } else if let Some(client_addr) = client_udp_addr {
                        if let Some(session_id) = udp_flows.direct_response_session_id(sender) {
                            self.record_session_outbound_rx(session_id, buf.len() as u64);
                            let sent = self
                                .forward_direct_udp_response(&relay, client_addr, sender, buf)
                                .await?;
                            self.record_session_inbound_tx(session_id, sent as u64);
                        } else {
                            self.forward_direct_udp_response(&relay, client_addr, sender, buf)
                                .await?;
                        }
                    } else {
                        debug!(?sender, "dropping udp packet from unexpected sender");
                    }
                }
                upstream = recv_upstream_packet(upstream_association.as_ref(), &mut upstream_packet) => {
                    match upstream {
                        Ok(read) => {
                            self.stats.record_udp_upstream_packet_received();
                            upstream_idle_deadline =
                                Some(TokioInstant::now() + self.udp_upstream_idle_timeout());
                            let mut session_id = None;
                            if let Some(association) = upstream_association.as_ref() {
                                match parse_udp_packet(&upstream_packet[..read]) {
                                    Ok(packet) => {
                                        session_id = udp_flows
                                            .upstream_response_session_id(
                                                association.outbound_tag(),
                                                &packet.target,
                                                packet.port,
                                            );
                                    }
                                    Err(error) => debug!(
                                        inbound_tag = inbound_tag,
                                        outbound_tag = association.outbound_tag(),
                                        error = %error,
                                        "failed to attribute upstream UDP response"
                                    ),
                                }
                            }
                            if let Some(client_addr) = client_udp_addr {
                                if let Some(session_id) = session_id {
                                    self.record_session_outbound_rx(session_id, read as u64);
                                }
                                let sent = relay
                                    .send_to_addr(&upstream_packet[..read], client_addr)
                                    .await?;
                                if let Some(session_id) = session_id {
                                    self.record_session_inbound_tx(session_id, sent as u64);
                                }
                            }
                        }
                        Err(error) => {
                            if let Some(association) = upstream_association.take() {
                                self.stats.record_udp_upstream_recv_failure();
                                upstream_idle_deadline = None;
                                let outbound_tag = association.outbound_tag().to_owned();
                                let (server, port) = association.upstream_endpoint();
                                let server = server.to_owned();
                                association.close(UpstreamAssociationCloseReason::Dropped);
                                log_udp_upstream_association_dropped(
                                    inbound_tag,
                                    &outbound_tag,
                                    server.as_str(),
                                    port,
                                    &error,
                                );
                            }
                        }
                    }
                }
                _ = wait_for_upstream_idle(upstream_idle_deadline), if upstream_association.is_some() => {
                    if let Some(association) = upstream_association.take() {
                        upstream_idle_deadline = None;
                        let outbound_tag = association.outbound_tag().to_owned();
                        let (server, port) = association.upstream_endpoint();
                        let server = server.to_owned();
                        association.close(UpstreamAssociationCloseReason::IdleTimeout);
                        log_udp_upstream_association_idle_timeout(
                            inbound_tag,
                            &outbound_tag,
                            server.as_str(),
                            port,
                            self.udp_upstream_idle_timeout(),
                        );
                    }
                }
            }
        }

        for completed in udp_flows.finish_all() {
            log_completed_udp_flow(completed);
        }

        if let Some(association) = upstream_association.take() {
            association.close(UpstreamAssociationCloseReason::Closed);
        }

        Ok(())
    }

    async fn handle_socks5_udp_request(
        &self,
        packet: &[u8],
        context: UdpRequestContext<'_>,
    ) -> Result<(), EngineError> {
        let udp_packet = parse_udp_packet(packet)?;

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
        self.prepare_session(&mut session, context.inbound_tag);
        let mut session_handle = self.track_session(session.id);
        let started_at = Instant::now();
        self.record_session_inbound_traffic(session.id, *context.pending_control_traffic);
        *context.pending_control_traffic = StreamTraffic::default();
        self.record_session_inbound_rx(session.id, packet.len() as u64);

        let action = self.route_decision(&session.target);
        let resolved = match self.resolve_outbound(action) {
            Ok(resolved) => resolved,
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
                    .map(|(server, port)| (server.as_str(), *port)),
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
                                    .map(|(server, port)| (server.as_str(), *port)),
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
                                    .map(|(server, port)| (server.as_str(), *port)),
                            );
                        }
                        return Err(error);
                    }
                }
            }
        }

        Ok(())
    }

    async fn start_udp_flow_candidate(
        &self,
        candidate: ResolvedLeafOutbound<'_>,
        context: UdpCandidateContext<'_>,
    ) -> Result<UdpCandidateStart, UdpCandidateFailure> {
        match candidate {
            ResolvedLeafOutbound::Direct { tag } => {
                let target_addr = self
                    .protocols
                    .direct_outbound
                    .resolve_target_addr(context.session, &self.resolver)
                    .await
                    .map_err(|error| UdpCandidateFailure {
                        stage: "resolve_udp_target",
                        error: error.into(),
                        upstream: None,
                    })?;

                let sent = context
                    .relay
                    .send_to_addr(context.payload, target_addr)
                    .await
                    .map_err(|error| UdpCandidateFailure {
                        stage: "udp_direct_send",
                        error: error.into(),
                        upstream: None,
                    })?;

                Ok(UdpCandidateStart::Flow {
                    outbound: UdpFlowOutbound::Direct {
                        tag: tag.unwrap_or("direct").to_owned(),
                        target_addr,
                    },
                    outbound_tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Block { tag } => Ok(UdpCandidateStart::Blocked {
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
                    .send_socks5_udp_packet(Socks5UdpPacketContext {
                        inbound_tag: context.inbound_tag,
                        tag,
                        server,
                        port,
                        auth: username.zip(password),
                        session: context.session,
                        payload: context.payload,
                        upstream_association: context.upstream_association,
                        upstream_idle_deadline: context.upstream_idle_deadline,
                    })
                    .await?;

                Ok(UdpCandidateStart::Flow {
                    outbound: UdpFlowOutbound::Socks5 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.map(ToOwned::to_owned),
                        password: password.map(ToOwned::to_owned),
                    },
                    outbound_tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Vless { server, port, .. } => Err(UdpCandidateFailure {
                stage: "udp_vless_outbound",
                error: zero_core::Error::Unsupported("VLESS UDP outbound is not supported").into(),
                upstream: Some((server.to_owned(), port)),
            }),
        }
    }

    async fn send_socks5_udp_packet(
        &self,
        context: Socks5UdpPacketContext<'_>,
    ) -> Result<usize, UdpCandidateFailure> {
        self.ensure_socks5_udp_association(
            context.inbound_tag,
            Socks5UdpAssociationEndpoint {
                tag: context.tag,
                server: context.server,
                port: context.port,
                auth: context.auth,
            },
            context.session.id,
            context.upstream_association,
            context.upstream_idle_deadline,
        )
        .await?;

        let association = context
            .upstream_association
            .as_ref()
            .expect("successful establish stores upstream association");

        let sent = match association
            .send_packet(
                &context.session.target,
                context.session.port,
                context.payload,
            )
            .await
        {
            Ok(sent) => sent,
            Err(error) => {
                self.stats.record_udp_upstream_send_failure();
                if let Some(association) = context.upstream_association.take() {
                    let outbound_tag = association.outbound_tag().to_owned();
                    association.close(UpstreamAssociationCloseReason::Dropped);
                    log_udp_upstream_association_dropped(
                        context.inbound_tag,
                        &outbound_tag,
                        context.server,
                        context.port,
                        &error,
                    );
                }
                *context.upstream_idle_deadline = None;
                return Err(UdpCandidateFailure {
                    stage: "udp_upstream_send",
                    error,
                    upstream: Some((context.server.to_owned(), context.port)),
                });
            }
        };

        self.stats.record_udp_upstream_packet_sent();
        *context.upstream_idle_deadline =
            Some(TokioInstant::now() + self.udp_upstream_idle_timeout());
        Ok(sent)
    }

    async fn ensure_socks5_udp_association(
        &self,
        inbound_tag: &str,
        endpoint: Socks5UdpAssociationEndpoint<'_>,
        session_id: u64,
        upstream_association: &mut Option<ActiveUpstreamSocks5UdpAssociation>,
        upstream_idle_deadline: &mut Option<TokioInstant>,
    ) -> Result<(), UdpCandidateFailure> {
        let needs_new_association = upstream_association
            .as_ref()
            .map(|association| !association.matches(endpoint.tag, endpoint.server, endpoint.port))
            .unwrap_or(true);

        if !needs_new_association {
            self.stats.record_udp_upstream_association_reused();
            log_udp_upstream_association_reused(
                inbound_tag,
                endpoint.tag,
                endpoint.server,
                endpoint.port,
            );
            return Ok(());
        }

        if let Some(association) = upstream_association.take() {
            association.close(UpstreamAssociationCloseReason::Closed);
            *upstream_idle_deadline = None;
        }

        match ActiveUpstreamSocks5UdpAssociation::establish(
            self,
            endpoint.tag,
            endpoint.server,
            endpoint.port,
            endpoint.auth,
            session_id,
        )
        .await
        {
            Ok(association) => {
                self.stats.record_udp_upstream_association_created();
                *upstream_idle_deadline =
                    Some(TokioInstant::now() + self.udp_upstream_idle_timeout());
                log_udp_upstream_association_created(
                    inbound_tag,
                    endpoint.tag,
                    endpoint.server,
                    endpoint.port,
                    self.udp_upstream_idle_timeout(),
                );
                *upstream_association = Some(association);
                Ok(())
            }
            Err(error) => {
                self.stats.record_udp_upstream_association_failed();
                Err(UdpCandidateFailure {
                    stage: "udp_upstream_associate",
                    error,
                    upstream: Some((endpoint.server.to_owned(), endpoint.port)),
                })
            }
        }
    }

    async fn forward_direct_udp_response(
        &self,
        relay: &TokioDatagramSocket,
        client_addr: SocketAddr,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        let packet = build_udp_packet(&address_from_socket_addr(sender), sender.port(), payload)?;
        relay
            .send_to_addr(&packet, client_addr)
            .await
            .map_err(EngineError::from)
    }
}

struct UdpCandidateContext<'a> {
    inbound_tag: &'a str,
    relay: &'a TokioDatagramSocket,
    session: &'a Session,
    payload: &'a [u8],
    upstream_association: &'a mut Option<ActiveUpstreamSocks5UdpAssociation>,
    upstream_idle_deadline: &'a mut Option<TokioInstant>,
}

struct UdpRequestContext<'a> {
    inbound_tag: &'a str,
    relay: &'a TokioDatagramSocket,
    udp_flows: &'a mut UdpSessionFlows,
    pending_control_traffic: &'a mut StreamTraffic,
    upstream_association: &'a mut Option<ActiveUpstreamSocks5UdpAssociation>,
    upstream_idle_deadline: &'a mut Option<TokioInstant>,
}

struct ExistingUdpFlowContext<'a> {
    inbound_tag: &'a str,
    relay: &'a TokioDatagramSocket,
    udp_flows: &'a mut UdpSessionFlows,
    upstream_association: &'a mut Option<ActiveUpstreamSocks5UdpAssociation>,
    upstream_idle_deadline: &'a mut Option<TokioInstant>,
}

struct Socks5UdpPacketContext<'a> {
    inbound_tag: &'a str,
    tag: &'a str,
    server: &'a str,
    port: u16,
    auth: Option<(&'a str, &'a str)>,
    session: &'a Session,
    payload: &'a [u8],
    upstream_association: &'a mut Option<ActiveUpstreamSocks5UdpAssociation>,
    upstream_idle_deadline: &'a mut Option<TokioInstant>,
}

#[derive(Debug, Clone, Copy)]
struct Socks5UdpAssociationEndpoint<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    auth: Option<(&'a str, &'a str)>,
}

enum UdpCandidateStart {
    Flow {
        outbound: UdpFlowOutbound,
        outbound_tx_bytes: u64,
    },
    Blocked {
        tag: String,
    },
}

struct UdpCandidateFailure {
    stage: &'static str,
    error: EngineError,
    upstream: Option<(String, u16)>,
}

fn log_completed_udp_flow(completed: CompletedUdpFlow) {
    log_session_finished(
        &completed.record,
        completed
            .upstream
            .as_ref()
            .map(|(server, port)| (server.as_str(), *port)),
    );
}

async fn recv_upstream_packet(
    association: Option<&ActiveUpstreamSocks5UdpAssociation>,
    buf: &mut [u8],
) -> Result<usize, EngineError> {
    match association {
        Some(association) => association.recv_packet(buf).await,
        None => std::future::pending::<Result<usize, EngineError>>().await,
    }
}

async fn wait_for_upstream_idle(deadline: Option<TokioInstant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

fn address_from_socket_addr(addr: SocketAddr) -> Address {
    match addr.ip() {
        IpAddr::V4(ip) => Address::Ipv4(ip.octets()),
        IpAddr::V6(ip) => Address::Ipv6(ip.octets()),
    }
}
