use tokio::select;
use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;
use tracing::warn;
use zero_core::Session;

use crate::inbound::udp_dispatch::dispatch_inbound_udp_packet;
use crate::inbound::udp_response::{
    write_chain_response_sync, write_direct_response_sync, write_upstream_response_sync,
};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_chain_udp_response_parts, record_direct_udp_response_parts,
    record_upstream_udp_response_received, wait_for_upstream_idle,
};
use crate::runtime::Proxy;

impl Proxy {
    pub(crate) fn spawn_vmess_mux_udp_stream_task(
        &self,
        tasks: &mut JoinSet<()>,
        mux_session_id: u16,
        session: Session,
        up_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>,
        writer: vmess::mux::VmessInboundMuxWriter,
        inbound_tag: String,
    ) {
        let mut up_rx = up_rx;
        let proxy = self.clone();
        tasks.spawn(async move {
            let mut udp_session = vmess::VmessInbound.udp_session_for(&session);
            let mut dispatch = match UdpDispatch::new(&inbound_tag).await {
                Ok(dispatch) => dispatch,
                Err(error) => {
                    warn!(%error, mux_session_id, "vmess mux udp dispatch init failed");
                    let _ = writer.end_inbound_stream(mux_session_id);
                    return;
                }
            };
            let timeout = proxy.udp_upstream_idle_timeout();
            let mut last_activity = TokioInstant::now();
            let mut direct_buf = vec![0_u8; 64 * 1024];
            let mut upstream_buf = vec![0_u8; 64 * 1024];

            loop {
                let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();
                select! {
                    _ = tokio::time::sleep_until(last_activity + timeout) => break,
                    payload = up_rx.recv() => {
                        let Some(payload) = payload else { break; };
                        if payload.is_empty() {
                            break;
                        }
                        last_activity = TokioInstant::now();
                        let inbound_dispatch = match udp_session.decode_mux_inbound_dispatch(&payload) {
                            Ok(inbound_dispatch) => inbound_dispatch,
                            Err(error) => {
                                warn!(%error, mux_session_id, "vmess mux udp packet parse failed");
                                break;
                            }
                        };
                        if let Err(error) =
                            dispatch_inbound_udp_packet(&proxy, &mut dispatch, &inbound_dispatch, None)
                                .await
                        {
                                warn!(%error, mux_session_id, "vmess mux udp packet dispatch failed");
                        }
                    }
                    recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                        match recv {
                            Ok((n, sender)) => {
                                last_activity = TokioInstant::now();
                                let response = record_direct_udp_response_parts(
                                    &proxy,
                                    &dispatch,
                                    sender,
                                    &direct_buf[..n],
                                );
                                match write_direct_response_sync(&response, || {
                                    udp_session.write_mux_client_response_for_target(
                                        &writer,
                                        mux_session_id,
                                        &response.target,
                                        response.port,
                                        response.payload,
                                    )
                                }) {
                                    Ok(_) => {}
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vmess mux udp direct response send failed");
                                        break;
                                    }
                                }
                            }
                            Err(error) => {
                                warn!(%error, mux_session_id, "vmess mux udp direct recv failed");
                                break;
                            }
                        }
                    }
                    upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                        match upstream {
                            Ok(pkt) => {
                                last_activity = TokioInstant::now();
                                let response = record_upstream_udp_response_received(
                                    &proxy,
                                    &mut dispatch,
                                    timeout,
                                    pkt,
                                );
                                match write_upstream_response_sync(&response, || {
                                    udp_session.write_mux_client_response_for_target(
                                        &writer,
                                        mux_session_id,
                                        &response.target,
                                        response.port,
                                        &response.payload,
                                    )
                                }) {
                                    Ok(_) => {}
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vmess mux udp upstream response send failed");
                                        break;
                                    }
                                }
                            }
                            Err(error) => warn!(%error, mux_session_id, "vmess mux udp socks5 upstream recv failed"),
                        }
                    }
                    _ = wait_for_upstream_idle(socks5_idle) => {}
                    Some(chain_result) = chain_tasks.join_next() => {
                        match chain_result {
                            Ok(Ok((target, port, payload, session_id))) => {
                                last_activity = TokioInstant::now();
                                let response =
                                    record_chain_udp_response_parts(&proxy, target, port, payload, session_id);
                                match write_chain_response_sync(&response, || {
                                    udp_session.write_mux_client_response_for_target(
                                        &writer,
                                        mux_session_id,
                                        &response.target,
                                        response.port,
                                        &response.payload,
                                    )
                                }) {
                                    Ok(_) => {}
                                    Err(error) => {
                                        warn!(%error, mux_session_id, "vmess mux udp chain response send failed");
                                        break;
                                    }
                                }
                            }
                            Ok(Err(error)) => warn!(%error, mux_session_id, "vmess mux udp chain response failed"),
                            Err(error) => warn!(%error, mux_session_id, "vmess mux udp chain task panicked"),
                        }
                    }
                }
            }

            for completed in dispatch.finish_all() {
                log_completed_udp_flow(completed);
            }
            let _ = writer.end_inbound_stream(mux_session_id);
        });
    }
}
