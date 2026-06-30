use tokio::select;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_chain_udp_response_received, record_direct_udp_response_parts,
    record_upstream_udp_response_received, wait_for_upstream_idle,
};
use crate::runtime::Proxy;

impl Proxy {
    pub(crate) async fn spawn_vless_mux_udp_stream_task(
        &self,
        mux_session_id: u16,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vless::mux::VlessInboundMuxWriter,
        inbound_tag: &str,
        auth: Option<&zero_core::SessionAuth>,
    ) {
        let mut up_rx = up_rx;
        let mut dispatch = match UdpDispatch::new(inbound_tag).await {
            Ok(dispatch) => dispatch,
            Err(error) => {
                warn!(%error, mux_session_id, "vless mux udp dispatch init failed");
                let _ = writer.end_inbound_stream(mux_session_id);
                return;
            }
        };
        let timeout = self.udp_upstream_idle_timeout();
        let mut last_activity = TokioInstant::now();
        let mut direct_buf = vec![0_u8; 64 * 1024];
        let mut upstream_buf = vec![0_u8; 64 * 1024];
        let udp_session = vless::VlessInbound.udp_session();

        info!(
            inbound_tag = inbound_tag,
            protocol = "vless_mux_udp",
            mux_session_id,
            "vless mux udp sub-stream started"
        );

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();
            select! {
                _ = tokio::time::sleep_until(last_activity + timeout) => {
                    info!(
                        inbound_tag = inbound_tag,
                        protocol = "vless_mux_udp",
                        mux_session_id,
                        "vless mux udp sub-stream idle timeout"
                    );
                    break;
                }
                payload = up_rx.recv() => {
                    let Some(payload) = payload else { break; };
                    if payload.is_empty() {
                        break;
                    }
                    last_activity = TokioInstant::now();
                    let inbound_dispatch = match udp_session.decode_mux_inbound_dispatch(&payload) {
                        Ok(inbound_dispatch) => inbound_dispatch,
                        Err(error) => {
                            warn!(%error, mux_session_id, "vless mux udp packet parse failed");
                            continue;
                        }
                    };
                    if let Err(error) = UdpPipe::new(self, &mut dispatch)
                        .dispatch(UdpPipeInput::from_inbound_dispatch(
                            &inbound_dispatch,
                            auth,
                        ))
                        .await
                    {
                        warn!(%error, mux_session_id, "vless mux udp packet dispatch failed");
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    match recv {
                        Ok((n, sender)) => {
                            last_activity = TokioInstant::now();
                            let response = record_direct_udp_response_parts(
                                self,
                                &dispatch,
                                sender,
                                &direct_buf[..n],
                            );
                            match udp_session.send_mux_client_response_for_target(
                                &writer,
                                mux_session_id,
                                &response.target,
                                response.port,
                                response.payload,
                            ) {
                                Ok(frame_len) => {
                                    response.accounting.record_sent(frame_len);
                                }
                                Err(error) => {
                                    warn!(%error, mux_session_id, "vless mux udp direct response encode failed");
                                    break;
                                }
                            }
                        }
                        Err(error) => {
                            warn!(%error, mux_session_id, "vless mux udp direct recv failed");
                            break;
                        }
                    }
                }
                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            last_activity = TokioInstant::now();
                            let response = record_upstream_udp_response_received(
                                self,
                                &mut dispatch,
                                timeout,
                                pkt,
                            );
                            match udp_session.send_mux_client_response_for_target(
                                &writer,
                                mux_session_id,
                                &response.target,
                                response.port,
                                &response.payload,
                            ) {
                                Ok(frame_len) => {
                                    response.accounting.record_sent(frame_len);
                                }
                                Err(error) => {
                                    warn!(%error, mux_session_id, "vless mux udp upstream response encode failed");
                                    break;
                                }
                            }
                        }
                        Err(error) => warn!(%error, mux_session_id, "vless mux udp socks5 upstream recv failed"),
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
                            match udp_session.send_mux_client_response_for_target(
                                &writer,
                                mux_session_id,
                                &target,
                                port,
                                &payload,
                            ) {
                                Ok(frame_len) => {
                                    response_accounting.record_sent(frame_len);
                                }
                                Err(error) => {
                                    warn!(%error, mux_session_id, "vless mux udp chain response encode failed");
                                    break;
                                }
                            }
                        }
                        Ok(Err(error)) => warn!(%error, mux_session_id, "vless mux udp chain response failed"),
                        Err(error) => warn!(%error, mux_session_id, "vless mux udp chain task panicked"),
                    }
                }
            }
        }

        for completed in dispatch.finish_all() {
            log_completed_udp_flow(completed);
        }
        let _ = writer.end_inbound_stream(mux_session_id);
    }
}
