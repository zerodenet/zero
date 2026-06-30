use tokio::select;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};

use crate::inbound::udp_dispatch::dispatch_inbound_udp_packet;
use crate::inbound::udp_response::{
    write_chain_response, write_direct_response, write_upstream_response,
};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_chain_udp_response_parts, record_direct_udp_response_parts,
    record_upstream_udp_response_received, wait_for_upstream_idle,
};
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
        let udp_responder = vless::VlessInbound.udp_responder();

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
                read = udp_responder.read_inbound_dispatch_tokio(&mut client, &mut buffer) => {
                    match read {
                        Ok(None) => break,
                        Ok(Some(inbound_dispatch)) => {
                            last_activity = TokioInstant::now();
                            self.record_session_inbound_traffic(0, client.drain_traffic());

                            if let Err(error) = dispatch_inbound_udp_packet(
                                &proxy,
                                &mut dispatch,
                                &inbound_dispatch,
                                auth.as_ref(),
                            )
                            .await
                            {
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

                    let response = record_direct_udp_response_parts(
                        self,
                        &dispatch,
                        sender,
                        &udp_buffer[..n],
                    );
                    match write_direct_response(&response, || async {
                        udp_responder
                            .write_response_for_target_tokio(
                                &mut client,
                                &response.target,
                                response.port,
                                response.payload,
                            )
                            .await
                    })
                    .await
                    {
                        Ok(_) => {
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
                upstream = upstream_udp.recv_response(&mut upstream_buffer) => {
                    // Registered upstream response - re-encode as VLESS.
                    match upstream {
                        Ok(pkt) => {
                            last_activity = TokioInstant::now();
                            let response = record_upstream_udp_response_received(
                                &proxy,
                                &mut dispatch,
                                timeout,
                                pkt,
                            );
                            match write_upstream_response(&response, || async {
                                udp_responder
                                    .write_response_for_target_tokio(
                                        &mut client,
                                        &response.target,
                                        response.port,
                                        &response.payload,
                                    )
                                    .await
                            })
                            .await
                            {
                                Ok(_) => {
                                    proxy.record_session_inbound_traffic(0, client.drain_traffic());
                                }
                                Err(error) => {
                                    warn!(error = %error, "failed to write vless udp upstream response");
                                    break;
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
                            let response =
                                record_chain_udp_response_parts(&proxy, target, port, payload, session_id);
                            match write_chain_response(&response, || async {
                                udp_responder
                                    .write_response_for_target_tokio(
                                        &mut client,
                                        &response.target,
                                        response.port,
                                        &response.payload,
                                    )
                                    .await
                            })
                            .await
                            {
                                Ok(_) => {
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
}
