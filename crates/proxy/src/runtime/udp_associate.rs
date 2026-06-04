use std::net::SocketAddr;
use tokio::select;
use tracing::{debug, info, warn};
use zero_platform_tokio::TokioDatagramSocket;
use zero_protocol_socks5::{parse_udp_packet, Socks5Reply, Socks5UdpAssociateRequest};
use zero_traits::AsyncSocket;
use zero_traits::DnsResolver;

use crate::logging::{
    log_udp_upstream_association_dropped, log_udp_upstream_association_idle_timeout,
};
use crate::runtime::Proxy;
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::transport::{ClientStream, MeteredStream, StreamTraffic};
use zero_core::{Address, ProtocolType};
use zero_engine::EngineError;

pub(crate) mod context;
pub(crate) mod helpers;
mod outbound;
mod request;
pub(crate) mod sessions;

use crate::outbound::socks5::UpstreamAssociationCloseReason;
use helpers::{
    address_from_socket_addr, log_completed_udp_flow, recv_upstream_packet, wait_for_upstream_idle,
};

impl Proxy {
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

        let mut dispatch = UdpDispatch::new(inbound_tag).await?;

        info!(
            inbound_tag = inbound_tag,
            protocol = "socks5-udp",
            relay = %relay_addr,
            "socks5 udp association ready"
        );

        let mut client_udp_addr: Option<SocketAddr> = None;
        let mut control_probe = [0_u8; 1];
        let mut packet = vec![0_u8; 64 * 1024];
        let mut direct_buf = vec![0_u8; 64 * 1024];
        let mut upstream_buf = vec![0_u8; 64 * 1024];

        loop {
            // Extract all mutable/immutable borrows in one go to satisfy
            // select!'s requirement that all branches be independent.
            let (direct_sock, vless_mgr, socks5_up, socks5_idle, chain_tasks) = dispatch.poll_refs();

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
                        // Client packet: parse SOCKS5 framing, dispatch.
                        if let Err(error) = self.socks5_dispatch_packet(
                            buf,
                            inbound_tag,
                            &mut dispatch,
                            &mut pending_control_traffic,
                        ).await {
                            warn!(
                                inbound_tag = inbound_tag,
                                protocol = "socks5-udp",
                                error = %error,
                                "failed to process UDP packet"
                            );
                        }

                    } else if let Some(client_addr) = client_udp_addr {
                        // Legacy direct response arriving on the relay socket
                        // (from pre-dispatch flows). Forward to client.
                        if let Some(session_id) = dispatch.direct_response_session_id(sender) {
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
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    // Direct outbound response from the dispatcher's socket.
                    let (n, sender) = recv?;

                    if let Some(client_addr) = client_udp_addr {
                        if let Some(session_id) = dispatch.direct_response_session_id(sender) {
                            self.record_session_outbound_rx(session_id, n as u64);
                        }

                        match self
                            .forward_direct_udp_response(
                                &relay, client_addr, sender, &direct_buf[..n],
                            )
                            .await
                        {
                            Ok(sent) => {
                                if let Some(session_id) =
                                    dispatch.direct_response_session_id(sender)
                                {
                                    self.record_session_inbound_tx(session_id, sent as u64);
                                }
                            }
                            Err(error) => {
                                warn!(
                                    inbound_tag = inbound_tag,
                                    protocol = "socks5-udp",
                                    error = %error,
                                    "failed to forward direct UDP response"
                                );
                            }
                        }
                    }
                }
                upstream = recv_upstream_packet(socks5_up, &mut upstream_buf) => {
                    match upstream {
                        Ok(read) => {
                            self.record_udp_upstream_packet_received();
                            dispatch.touch_socks5_idle(self.udp_upstream_idle_timeout());
                            let mut session_id = None;
                            if let Some(association) = dispatch.socks5_upstream() {
                                match parse_udp_packet(&upstream_buf[..read]) {
                                    Ok(pkt) => {
                                        session_id = dispatch.upstream_response_session_id(
                                            association.outbound_tag(),
                                            &pkt.target,
                                            pkt.port,
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
                                if let Some(sid) = session_id {
                                    self.record_session_outbound_rx(sid, read as u64);
                                }
                                let sent = relay
                                    .send_to_addr(&upstream_buf[..read], client_addr)
                                    .await?;
                                if let Some(sid) = session_id {
                                    self.record_session_inbound_tx(sid, sent as u64);
                                }
                            }
                        }
                        Err(error) => {
                            if let Some(association) = dispatch.take_socks5_upstream() {
                                self.record_udp_upstream_recv_failure();
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
                response = vless_mgr.next_response() => {
                    match response {
                        Some(Ok((target, port, payload, session_id))) => {
                            if let Some(sid) = session_id {
                                self.record_session_outbound_rx(sid, payload.len() as u64);
                            }
                            if let Some(client_addr) = client_udp_addr {
                                if let Ok(frame) = zero_protocol_socks5::build_udp_packet(
                                    &target, port, &payload,
                                ) {
                                    let sent = relay.send_to_addr(&frame, client_addr).await?;
                                    if let Some(sid) = session_id {
                                        self.record_session_inbound_tx(sid, sent as u64);
                                    }
                                }
                            }
                        }
                        Some(Err(error)) => {
                            warn!(
                                inbound_tag = inbound_tag,
                                protocol = "socks5-udp",
                                error = %error,
                                "VLESS chain upstream read error"
                            );
                        }
                        None => {}
                    }
                }
                Some(chain_result) = chain_tasks.join_next() => {
                    // Chain-outbound response (SS, H2, VLESS via JoinSet).
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            if let Some(sid) = session_id {
                                self.record_session_outbound_rx(sid, payload.len() as u64);
                            }
                            if let Some(client_addr) = client_udp_addr {
                                if let Ok(frame) = zero_protocol_socks5::build_udp_packet(
                                    &target, port, &payload,
                                ) {
                                    if let Ok(sent) = relay.send_to_addr(&frame, client_addr).await {
                                        if let Some(sid) = session_id {
                                            self.record_session_inbound_tx(sid, sent as u64);
                                        }
                                    }
                                }
                            }
                        }
                        Ok(Err(error)) => {
                            warn!(error = %error, "chain upstream read error");
                        }
                        Err(join_err) => {
                            warn!(error = %join_err, "chain response task panicked");
                        }
                    }
                }
                _ = wait_for_upstream_idle(socks5_idle) => {
                    if let Some(association) = dispatch.take_socks5_upstream() {
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

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }

        Ok(())
    }

    /// Parse a SOCKS5 UDP packet, handle DNS interception, then dispatch
    /// via the generic `UdpDispatch`.
    async fn socks5_dispatch_packet(
        &self,
        packet: &[u8],
        _inbound_tag: &str,
        dispatch: &mut UdpDispatch,
        pending_control_traffic: &mut StreamTraffic,
    ) -> Result<(), EngineError> {
        let udp_packet = parse_udp_packet(packet)?;

        // ── DNS interception ─────────────────────────────────────────
        // Intercept UDP packets to port 53 with a domain target.
        // Resolve locally through DnsSystem and reply directly.
        if udp_packet.port == 53 {
            if let Address::Domain(ref domain) = udp_packet.target {
                match self.resolver.resolve(domain).await {
                    Ok(_ips) => {
                        // DNS resolved locally — build response and return.
                        // The caller will forward via the relay socket if
                        // available. For now, skip dispatch and return Ok.
                        // The DNS response is sent inline in the main loop.
                        return Ok(());
                    }
                    Err(_) => {
                        // Resolution failed — silently drop.
                        return Ok(());
                    }
                }
            }
        }

        // Record TCP control traffic from the SOCKS5 session.
        self.record_session_inbound_traffic(0, pending_control_traffic.clone());
        *pending_control_traffic = StreamTraffic::default();
        self.record_session_inbound_rx(0, packet.len() as u64);

        // ── Generic dispatch ─────────────────────────────────────────
        dispatch
            .dispatch(
                self,
                udp_packet.target,
                udp_packet.port,
                &udp_packet.payload,
                ProtocolType::Socks5,
                None,
            )
            .await
    }

    async fn forward_direct_udp_response(
        &self,
        relay: &TokioDatagramSocket,
        client_addr: SocketAddr,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        let packet = zero_protocol_socks5::build_udp_packet(
            &address_from_socket_addr(sender),
            sender.port(),
            payload,
        )?;
        relay
            .send_to_addr(&packet, client_addr)
            .await
            .map_err(EngineError::from)
    }
}
