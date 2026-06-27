use tokio::select;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_traits::AsyncSocket;

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{log_completed_udp_flow, wait_for_upstream_idle};
use crate::runtime::Proxy;
use crate::transport::{ClientStream, MeteredStream};
use zero_engine::EngineError;

impl Proxy {
    pub(crate) async fn handle_vless_udp_session<S>(
        &self,
        mut client: MeteredStream<S>,
        inbound_tag: &str,
        session: zero_core::Session,
        auth: &Option<zero_core::SessionAuth>,
    ) -> Result<(), EngineError>
    where
        S: ClientStream,
    {
        vless::VlessInbound.send_response(&mut client).await?;
        self.record_session_inbound_traffic(session.id, client.drain_traffic());

        let mut dispatch = UdpDispatch::new(inbound_tag).await?;
        let mut last_activity = TokioInstant::now();
        let timeout = self.udp_upstream_idle_timeout();

        info!(
            inbound_tag = inbound_tag,
            protocol = "vless_udp",
            "vless udp session started"
        );

        let mut buffer = vec![0_u8; 64 * 1024];
        let mut udp_buffer = vec![0_u8; 64 * 1024];
        let mut upstream_buffer = vec![0_u8; 64 * 1024];
        let proxy = self.clone();

        loop {
            // Split dispatch into disjoint borrows so select! can pin
            // all futures simultaneously without borrow conflicts.
            // VLESS chain responses now flow through chain_tasks.JoinSet
            // alongside SS/H2/Trojan/Mieru -?no separate vless_mgr poll.
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

            select! {
                _ = tokio::time::sleep_until(last_activity + timeout) => {
                    info!(
                        inbound_tag = inbound_tag,
                        protocol = "vless_udp",
                        "vless udp session idle timeout"
                    );
                    break;
                }
                read = client.read(&mut buffer) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            last_activity = TokioInstant::now();
                            self.record_session_inbound_traffic(0, client.drain_traffic());

                            if let Err(error) = Self::vless_dispatch_packet(
                                &proxy,
                                &mut dispatch,
                                &buffer[..n],
                                auth,
                            ).await {
                                warn!(
                                    error = %error,
                                    "failed to process vless udp packet"
                                );
                            }

                        }
                        Err(error) => {
                            warn!(
                                error = %error,
                                "vless udp client read error"
                            );
                            break;
                        }
                    }
                }
                recv = direct_sock.recv_from_addr(&mut udp_buffer) => {
                    let (n, sender) = recv?;
                    last_activity = TokioInstant::now();

                    let ip = zero_platform_tokio::socket_addr_to_ip(sender);
                    let target = match ip {
                        zero_traits::IpAddress::V4(bytes) => zero_core::Address::Ipv4(bytes),
                        zero_traits::IpAddress::V6(bytes) => zero_core::Address::Ipv6(bytes),
                    };
                    let port = sender.port();

                    if let Some(session_id) = dispatch.direct_response_session_id(sender) {
                        self.record_session_outbound_rx(session_id, n as u64);
                    }

                    match vless::VlessInboundUdpCodec.write_response_tokio(
                        &mut client,
                        &target,
                        port,
                        &udp_buffer[..n],
                    ).await {
                        Ok(written) => {
                            if let Some(session_id) = dispatch.direct_response_session_id(sender) {
                                self.record_session_inbound_tx(session_id, written as u64);
                            }
                            self.record_session_inbound_traffic(0, client.drain_traffic());
                        }
                        Err(error) => {
                            warn!(
                                error = %error,
                                "failed to write vless udp response"
                            );
                            break;
                        }
                    }
                }
                upstream = upstream_udp.recv_packet(&mut upstream_buffer) => {
                    // SOCKS5 chain upstream response -?re-encode as VLESS.
                    match upstream {
                        Ok(read) => {
                            last_activity = TokioInstant::now();
                            if let Ok(pkt) = socks5::decode_udp_associate_response(&upstream_buffer[..read]) {
                                if vless::VlessInboundUdpCodec.write_response_tokio(
                                    &mut client,
                                    &pkt.target,
                                    pkt.port,
                                    &pkt.payload,
                                ).await.is_ok() {
                                    proxy.record_session_inbound_traffic(0, client.drain_traffic());
                                }
                            }
                        }
                        Err(error) => {
                            warn!(error = %error, "SOCKS5 upstream recv error");
                        }
                    }
                }
                _ = wait_for_upstream_idle(socks5_idle) => {
                    // SOCKS5 upstream idle timeout -?association will be
                    // closed by finish_all() on session end.
                }
                Some(chain_result) = chain_tasks.join_next() => {
                    // Chain-outbound response (SS, H2, VLESS via JoinSet).
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            last_activity = TokioInstant::now();
                            if let Some(sid) = session_id {
                                proxy.record_session_outbound_rx(sid, payload.len() as u64);
                            }
                            match vless::VlessInboundUdpCodec.write_response_tokio(
                                &mut client,
                                &target,
                                port,
                                &payload,
                            ).await {
                                Ok(written) => {
                                    if let Some(sid) = session_id {
                                        proxy.record_session_inbound_tx(sid, written as u64);
                                    }
                                    proxy.record_session_inbound_traffic(0, client.drain_traffic());
                                }
                                Err(error) => {
                                    warn!(error = %error, "failed to write chain response");
                                    break;
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
            }
        }

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }

        info!(
            inbound_tag = inbound_tag,
            protocol = "vless_udp",
            "vless udp session ended"
        );

        Ok(())
    }

    /// Parse a VLESS UDP packet and dispatch via the UDP kernel pipe.
    pub(crate) async fn vless_dispatch_packet(
        proxy: &Proxy,
        dispatch: &mut UdpDispatch,
        packet: &[u8],
        auth: &Option<zero_core::SessionAuth>,
    ) -> Result<(), EngineError> {
        let udp_packet = vless::VlessInboundUdpCodec.decode_datagram(packet)?;

        UdpPipe::new(proxy, dispatch)
            .dispatch(UdpPipeInput {
                target: udp_packet.target,
                port: udp_packet.port,
                payload: &udp_packet.payload,
                protocol: zero_core::ProtocolType::Vless,
                auth: auth.as_ref(),
                client_session_id: None,
            })
            .await
            .map(|_| ())
    }
}
