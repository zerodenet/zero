use tokio::select;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_core::Session;
use zero_engine::EngineError;

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
use crate::transport::TcpRelayStream;

impl Proxy {
    pub(super) async fn run_trojan_udp_relay(
        &self,
        mut client: TcpRelayStream,
        session: Session,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let mut dispatch = UdpDispatch::new(inbound_tag).await?;
        let auth = session.auth.clone();
        let mut last_activity = TokioInstant::now();
        let timeout = self.udp_upstream_idle_timeout();
        let udp_session = trojan::TrojanInbound.udp_session();

        info!(
            inbound_tag = inbound_tag,
            protocol = "trojan_udp",
            "trojan udp session started"
        );

        let mut direct_buf = vec![0_u8; 64 * 1024];
        let mut upstream_buf = vec![0_u8; 64 * 1024];

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

            select! {
                _ = tokio::time::sleep_until(last_activity + timeout) => {
                    info!(
                        inbound_tag = inbound_tag,
                        protocol = "trojan_udp",
                        "trojan udp session idle timeout"
                    );
                    break;
                }
                packet = udp_session.read_inbound_dispatch(&mut client) => {
                    match packet {
                        Ok(inbound_dispatch) => {
                            last_activity = TokioInstant::now();
                            if let Err(error) = dispatch_inbound_udp_packet(
                                self,
                                &mut dispatch,
                                &inbound_dispatch,
                                auth.as_ref(),
                            )
                            .await
                            {
                                warn!(error = %error, "failed to process trojan udp packet");
                            }
                        }
                        Err(error) => {
                            warn!(error = %error, "trojan udp client read error");
                            break;
                        }
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    last_activity = TokioInstant::now();

                    let response = record_direct_udp_response_parts(
                        self,
                        &dispatch,
                        sender,
                        &direct_buf[..n],
                    );
                    write_direct_response(&response, || async {
                        udp_session
                            .write_client_response_for_target(
                                &mut client,
                                &response.target,
                                response.port,
                                response.payload,
                            )
                            .await
                    })
                    .await?;
                }
                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            last_activity = TokioInstant::now();
                            let response = record_upstream_udp_response_received(
                                self,
                                &mut dispatch,
                                self.udp_upstream_idle_timeout(),
                                pkt,
                            );
                            write_upstream_response(&response, || async {
                                udp_session
                                    .write_client_response_for_target(
                                        &mut client,
                                        &response.target,
                                        response.port,
                                        &response.payload,
                                    )
                                    .await
                            })
                            .await?;
                        }
                        Err(error) => {
                            warn!(error = %error, "trojan udp socks5 upstream recv error");
                        }
                    }
                }
                _ = wait_for_upstream_idle(socks5_idle) => {}
                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            last_activity = TokioInstant::now();
                            let response =
                                record_chain_udp_response_parts(self, target, port, payload, session_id);
                            write_chain_response(&response, || async {
                                udp_session
                                    .write_client_response_for_target(
                                        &mut client,
                                        &response.target,
                                        response.port,
                                        &response.payload,
                                    )
                                    .await
                            })
                            .await?;
                        }
                        Ok(Err(error)) => warn!(error = %error, "trojan udp chain response error"),
                        Err(error) => warn!(error = %error, "trojan udp chain task panicked"),
                    }
                }
            }
        }

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }

        info!(
            inbound_tag = inbound_tag,
            protocol = "trojan_udp",
            "trojan udp session ended"
        );

        Ok(())
    }
}
