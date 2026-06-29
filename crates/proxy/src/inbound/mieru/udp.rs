use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_core::Session;
use zero_engine::EngineError;

use super::MieruClientStream;
use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_chain_udp_response_received,
    record_direct_udp_response_received, record_upstream_udp_response_received,
    udp_response_target_from_socket_addr, wait_for_upstream_idle,
};
use crate::runtime::Proxy;

impl Proxy {
    /// Run a Mieru UDP relay through the generic UDP pipe.
    pub(super) async fn run_mieru_udp_relay(
        &self,
        mut client: MieruClientStream,
        session: &Session,
        inbound_tag: &str,
    ) -> Result<(), EngineError> {
        let mut dispatch = crate::runtime::udp_dispatch::UdpDispatch::new(inbound_tag).await?;
        let auth = session.auth.clone();
        let mut last_activity = TokioInstant::now();
        let timeout = self.udp_upstream_idle_timeout();

        let mut read_buf = [0u8; 65536];
        let mut direct_buf = [0u8; 65536];
        let mut upstream_buf = [0u8; 65536];
        let udp_session = mieru::MieruInbound.udp_session();

        info!(
            inbound_tag = inbound_tag,
            protocol = "mieru_udp",
            "mieru udp session started"
        );

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

            tokio::select! {
                _ = tokio::time::sleep_until(last_activity + timeout) => {
                    info!(
                        inbound_tag = inbound_tag,
                        protocol = "mieru_udp",
                        "mieru udp session idle timeout"
                    );
                    break;
                }
                read = udp_session.read_inbound_dispatch_tokio(&mut client, &mut read_buf) => {
                    match read {
                        Ok(None) => break,
                        Ok(Some(inbound_dispatch)) => {
                            last_activity = TokioInstant::now();
                            if let Err(error) = UdpPipe::new(self, &mut dispatch)
                                .dispatch(UdpPipeInput::from_inbound_dispatch(
                                    &inbound_dispatch,
                                    auth.as_ref(),
                                ))
                                .await
                            {
                                warn!(error = %error, "failed to process mieru udp packet");
                            }
                        }
                        Err(error) => {
                            warn!(error = %error, "mieru udp request read/decode failed");
                            break;
                        }
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    last_activity = TokioInstant::now();

                    let response_accounting =
                        record_direct_udp_response_received(self, &dispatch, sender, n);
                    let (target, port) = udp_response_target_from_socket_addr(sender);
                    let written = udp_session
                        .write_client_response_for_target_tokio(
                            &mut client,
                            &target,
                            port,
                            &direct_buf[..n],
                        )
                        .await?;
                    response_accounting.record_sent(written);
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
                            let written = udp_session
                                .write_client_response_for_target_tokio(
                                    &mut client,
                                    &response.target,
                                    response.port,
                                    &response.payload,
                                )
                                .await?;
                            response.accounting.record_sent(written);
                        }
                        Err(error) => {
                            warn!(error = %error, "mieru udp socks5 upstream recv error");
                        }
                    }
                }
                _ = wait_for_upstream_idle(socks5_idle) => {}
                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            last_activity = TokioInstant::now();
                            let response_accounting =
                                record_chain_udp_response_received(
                                    self,
                                    session_id,
                                    payload.len(),
                                );
                            let written = udp_session
                                .write_client_response_for_target_tokio(
                                    &mut client,
                                    &target,
                                    port,
                                    &payload,
                                )
                                .await?;
                            response_accounting.record_sent(written);
                        }
                        Ok(Err(error)) => warn!(error = %error, "mieru udp chain response error"),
                        Err(error) => warn!(error = %error, "mieru udp chain task panicked"),
                    }
                }
            }
        }

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }

        info!(inbound_tag = %inbound_tag, "mieru udp session ended");
        Ok(())
    }
}
