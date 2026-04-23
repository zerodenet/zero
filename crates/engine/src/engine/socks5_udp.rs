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

use super::error::EngineError;
use super::logging::{
    log_session_accepted, log_session_failed, log_session_finished,
    log_udp_upstream_association_created, log_udp_upstream_association_dropped,
    log_udp_upstream_association_idle_timeout, log_udp_upstream_association_reused,
};
use super::resolve::{ResolvedLeafOutbound, ResolvedOutbound};
use super::runtime::Engine;
use super::session_lifecycle::SessionHandle;
use super::stats::SessionOutcome;
use super::stream::ClientStream;
use super::upstream_socks5_udp::{
    ActiveUpstreamSocks5UdpAssociation, UpstreamAssociationCloseReason,
};

impl Engine {
    pub(crate) async fn handle_socks5_udp_associate<S>(
        &self,
        mut client: S,
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

        info!(
            inbound_tag = inbound_tag,
            protocol = "socks5-udp",
            relay = %relay_addr,
            "socks5 udp association ready"
        );

        let mut client_udp_addr: Option<SocketAddr> = None;
        let mut upstream_association: Option<ActiveUpstreamSocks5UdpAssociation> = None;
        let mut upstream_idle_deadline: Option<TokioInstant> = None;
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
                            inbound_tag,
                            &relay,
                            buf,
                            &mut upstream_association,
                            &mut upstream_idle_deadline,
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
                        self.forward_direct_udp_response(&relay, client_addr, sender, buf)
                            .await?;
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
                            if let Some(client_addr) = client_udp_addr {
                                relay
                                    .send_to_addr(&upstream_packet[..read], client_addr)
                                    .await?;
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

        if let Some(association) = upstream_association.take() {
            association.close(UpstreamAssociationCloseReason::Closed);
        }

        Ok(())
    }

    async fn handle_socks5_udp_request(
        &self,
        inbound_tag: &str,
        relay: &TokioDatagramSocket,
        packet: &[u8],
        upstream_association: &mut Option<ActiveUpstreamSocks5UdpAssociation>,
        upstream_idle_deadline: &mut Option<TokioInstant>,
    ) -> Result<(), EngineError> {
        let udp_packet = parse_udp_packet(packet)?;
        let mut session = Session::new(
            0,
            udp_packet.target,
            udp_packet.port,
            Network::Udp,
            ProtocolType::Socks5,
        );
        self.prepare_session(&mut session, inbound_tag);
        let mut session_handle = self.track_session(session.id);
        let started_at = Instant::now();

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

        match resolved {
            ResolvedOutbound::Single(candidate) => {
                self.process_socks5_udp_candidate(
                    candidate,
                    UdpCandidateContext {
                        inbound_tag,
                        relay,
                        session: &session,
                        payload: &udp_packet.payload,
                        upstream_association,
                        upstream_idle_deadline,
                        session_handle: &mut session_handle,
                        started_at,
                    },
                )
                .await?;
            }
            ResolvedOutbound::Fallback { candidates } => {
                let mut handled = false;

                for candidate in candidates {
                    match self
                        .process_socks5_udp_candidate(
                            candidate,
                            UdpCandidateContext {
                                inbound_tag,
                                relay,
                                session: &session,
                                payload: &udp_packet.payload,
                                upstream_association,
                                upstream_idle_deadline,
                                session_handle: &mut session_handle,
                                started_at,
                            },
                        )
                        .await
                    {
                        Ok(true) => {
                            handled = true;
                            break;
                        }
                        Ok(false) => {}
                        Err(error) => return Err(error),
                    }
                }

                if !handled {
                    let record = session_handle.finish(SessionOutcome::Failed);
                    log_session_failed(
                        &session,
                        record.as_ref(),
                        "fallback_exhausted",
                        started_at.elapsed(),
                        &EngineError::Io(std::io::Error::other("all fallback outbounds failed")),
                        None,
                    );
                }
            }
        }

        Ok(())
    }

    async fn process_socks5_udp_candidate(
        &self,
        candidate: ResolvedLeafOutbound<'_>,
        context: UdpCandidateContext<'_>,
    ) -> Result<bool, EngineError> {
        let UdpCandidateContext {
            inbound_tag,
            relay,
            session,
            payload,
            upstream_association,
            upstream_idle_deadline,
            session_handle,
            started_at,
        } = context;

        match candidate {
            ResolvedLeafOutbound::Direct { tag } => {
                let mut session = session.clone();
                session.outbound_tag = Some(tag.unwrap_or("direct").to_owned());
                self.set_session_outbound(&session);
                let target_addr = match self
                    .protocols
                    .direct_outbound
                    .resolve_target_addr(&session, &self.resolver)
                    .await
                {
                    Ok(addr) => addr,
                    Err(error) => {
                        log_session_failed(
                            &session,
                            None,
                            "resolve_udp_target",
                            started_at.elapsed(),
                            &error,
                            None,
                        );
                        return Ok(false);
                    }
                };

                relay
                    .send_to_addr(payload, target_addr)
                    .await
                    .map_err(EngineError::from)?;

                self.record_session_upload(session.id, payload.len() as u64);
                if let Some(record) = session_handle.finish(SessionOutcome::DirectRelayed) {
                    log_session_finished(&record, None);
                }

                Ok(true)
            }
            ResolvedLeafOutbound::Block { tag } => {
                let mut session = session.clone();
                session.outbound_tag = Some(tag.unwrap_or("block").to_owned());
                self.set_session_outbound(&session);
                if let Some(record) = session_handle.finish(SessionOutcome::Blocked) {
                    log_session_finished(&record, None);
                }

                Ok(true)
            }
            ResolvedLeafOutbound::Socks5 { tag, server, port } => {
                let mut session = session.clone();
                session.outbound_tag = Some(tag.to_owned());
                self.set_session_outbound(&session);
                let needs_new_association = upstream_association
                    .as_ref()
                    .map(|association| !association.matches(tag, server, port))
                    .unwrap_or(true);
                if needs_new_association {
                    if let Some(association) = upstream_association.take() {
                        association.close(UpstreamAssociationCloseReason::Closed);
                        *upstream_idle_deadline = None;
                    }

                    *upstream_association = match ActiveUpstreamSocks5UdpAssociation::establish(
                        self, tag, server, port,
                    )
                    .await
                    {
                        Ok(association) => {
                            self.stats.record_udp_upstream_association_created();
                            *upstream_idle_deadline =
                                Some(TokioInstant::now() + self.udp_upstream_idle_timeout());
                            log_udp_upstream_association_created(
                                inbound_tag,
                                tag,
                                server,
                                port,
                                self.udp_upstream_idle_timeout(),
                            );
                            Some(association)
                        }
                        Err(error) => {
                            self.stats.record_udp_upstream_association_failed();
                            log_session_failed(
                                &session,
                                None,
                                "udp_upstream_associate",
                                started_at.elapsed(),
                                &error,
                                Some((server, port)),
                            );
                            return Ok(false);
                        }
                    };
                } else {
                    self.stats.record_udp_upstream_association_reused();
                    log_udp_upstream_association_reused(inbound_tag, tag, server, port);
                }
                let association = upstream_association
                    .as_ref()
                    .expect("successful establish stores upstream association");

                if let Err(error) = association
                    .send_packet(&session.target, session.port, payload)
                    .await
                {
                    self.stats.record_udp_upstream_send_failure();
                    log_session_failed(
                        &session,
                        None,
                        "udp_upstream_send",
                        started_at.elapsed(),
                        &error,
                        Some((server, port)),
                    );
                    if let Some(association) = upstream_association.take() {
                        let outbound_tag = association.outbound_tag().to_owned();
                        association.close(UpstreamAssociationCloseReason::Dropped);
                        log_udp_upstream_association_dropped(
                            inbound_tag,
                            &outbound_tag,
                            server,
                            port,
                            &error,
                        );
                    }
                    *upstream_idle_deadline = None;
                    return Ok(false);
                }
                self.stats.record_udp_upstream_packet_sent();
                *upstream_idle_deadline =
                    Some(TokioInstant::now() + self.udp_upstream_idle_timeout());

                self.record_session_upload(session.id, payload.len() as u64);
                if let Some(record) = session_handle.finish(SessionOutcome::ChainedRelayed) {
                    log_session_finished(&record, Some((server, port)));
                }

                Ok(true)
            }
        }
    }

    async fn forward_direct_udp_response(
        &self,
        relay: &TokioDatagramSocket,
        client_addr: SocketAddr,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let packet = build_udp_packet(&address_from_socket_addr(sender), sender.port(), payload)?;
        relay.send_to_addr(&packet, client_addr).await?;
        Ok(())
    }
}

struct UdpCandidateContext<'a> {
    inbound_tag: &'a str,
    relay: &'a TokioDatagramSocket,
    session: &'a Session,
    payload: &'a [u8],
    upstream_association: &'a mut Option<ActiveUpstreamSocks5UdpAssociation>,
    upstream_idle_deadline: &'a mut Option<TokioInstant>,
    session_handle: &'a mut SessionHandle,
    started_at: Instant,
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
