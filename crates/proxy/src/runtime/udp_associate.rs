use std::net::SocketAddr;
use tokio::select;
use tokio::time::Instant as TokioInstant;
use tracing::{debug, info, warn};
use zero_platform_tokio::TokioDatagramSocket;
use zero_protocol_socks5::{parse_udp_packet, Socks5Reply, Socks5UdpAssociateRequest};
use zero_traits::AsyncSocket;

use crate::logging::{
    log_udp_upstream_association_dropped, log_udp_upstream_association_idle_timeout,
};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

pub(crate) mod context;
pub(crate) mod helpers;
mod outbound;
mod request;
pub(crate) mod sessions;

use context::UdpRequestContext;
use helpers::{
    address_from_socket_addr, log_completed_udp_flow, recv_upstream_packet, wait_for_upstream_idle,
};
use sessions::UdpSessionFlows;
use crate::outbound::socks5::{ActiveUpstreamSocks5UdpAssociation, UpstreamAssociationCloseReason};

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
                                client_addr: client_udp_addr,
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

                        // Forward pending SS upstream responses to the SOCKS5 client.
                        #[cfg(feature = "outbound-shadowsocks")]
                        if let Some(client_addr) = client_udp_addr {
                            use crate::outbound::shadowsocks::drain_all_responses;
                            for resp in drain_all_responses() {
                                if let Ok(frame) = zero_protocol_socks5::build_udp_packet(
                                    &resp.target, resp.port, &resp.payload,
                                ) {
                                    let _ = relay.send_to_addr(&frame, client_addr).await;
                                }
                            }
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
                            self.record_udp_upstream_packet_received();
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
                                self.record_udp_upstream_recv_failure();
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
