use tokio::select;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_core::{InboundUdpDispatch, SessionAuth};

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

pub(crate) enum MuxUdpDecodeFailure {
    Continue,
    End,
}

pub(crate) trait MuxUdpResponder: Send {
    fn decode_inbound_dispatch(
        &mut self,
        payload: &[u8],
    ) -> Result<InboundUdpDispatch, zero_core::Error>;

    fn write_response_for_target(
        &mut self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error>;

    fn end_inbound_stream(&mut self) -> Result<usize, zero_core::Error>;

    fn decode_failure(&self) -> MuxUdpDecodeFailure {
        MuxUdpDecodeFailure::End
    }
}

pub(crate) struct MuxUdpRelayRequest<'a, R> {
    pub(crate) mux_session_id: u16,
    pub(crate) up_rx: UnboundedReceiver<Vec<u8>>,
    pub(crate) responder: R,
    pub(crate) inbound_tag: &'a str,
    pub(crate) protocol: &'static str,
    pub(crate) auth: Option<&'a SessionAuth>,
}

pub(crate) async fn run_mux_udp_relay<R>(proxy: &Proxy, request: MuxUdpRelayRequest<'_, R>)
where
    R: MuxUdpResponder,
{
    let MuxUdpRelayRequest {
        mux_session_id,
        mut up_rx,
        mut responder,
        inbound_tag,
        protocol,
        auth,
    } = request;

    let mut dispatch = match UdpDispatch::new(inbound_tag).await {
        Ok(dispatch) => dispatch,
        Err(error) => {
            warn!(%error, mux_session_id, protocol, "mux udp dispatch init failed");
            let _ = responder.end_inbound_stream();
            return;
        }
    };
    let timeout = proxy.udp_upstream_idle_timeout();
    let mut last_activity = TokioInstant::now();
    let mut direct_buf = vec![0_u8; 64 * 1024];
    let mut upstream_buf = vec![0_u8; 64 * 1024];

    info!(
        inbound_tag = inbound_tag,
        protocol = protocol,
        mux_session_id,
        "mux udp sub-stream started"
    );

    loop {
        let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();
        select! {
            _ = tokio::time::sleep_until(last_activity + timeout) => {
                info!(
                    inbound_tag = inbound_tag,
                    protocol = protocol,
                    mux_session_id,
                    "mux udp sub-stream idle timeout"
                );
                break;
            }
            payload = up_rx.recv() => {
                let Some(payload) = payload else { break; };
                if payload.is_empty() {
                    break;
                }
                last_activity = TokioInstant::now();
                let inbound_dispatch = match responder.decode_inbound_dispatch(&payload) {
                    Ok(inbound_dispatch) => inbound_dispatch,
                    Err(error) => {
                        warn!(%error, mux_session_id, protocol, "mux udp packet parse failed");
                        match responder.decode_failure() {
                            MuxUdpDecodeFailure::Continue => continue,
                            MuxUdpDecodeFailure::End => break,
                        }
                    }
                };
                if let Err(error) =
                    dispatch_inbound_udp_packet(proxy, &mut dispatch, &inbound_dispatch, auth).await
                {
                    warn!(%error, mux_session_id, protocol, "mux udp packet dispatch failed");
                }
            }
            recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                match recv {
                    Ok((n, sender)) => {
                        last_activity = TokioInstant::now();
                        let response = record_direct_udp_response_parts(
                            proxy,
                            &dispatch,
                            sender,
                            &direct_buf[..n],
                        );
                        match write_direct_response_sync(&response, || {
                            responder.write_response_for_target(
                                &response.target,
                                response.port,
                                response.payload,
                            )
                        }) {
                            Ok(_) => {}
                            Err(error) => {
                                warn!(%error, mux_session_id, protocol, "mux udp direct response encode failed");
                                break;
                            }
                        }
                    }
                    Err(error) => {
                        warn!(%error, mux_session_id, protocol, "mux udp direct recv failed");
                        break;
                    }
                }
            }
            upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                match upstream {
                    Ok(pkt) => {
                        last_activity = TokioInstant::now();
                        let response = record_upstream_udp_response_received(
                            proxy,
                            &mut dispatch,
                            timeout,
                            pkt,
                        );
                        match write_upstream_response_sync(&response, || {
                            responder.write_response_for_target(
                                &response.target,
                                response.port,
                                &response.payload,
                            )
                        }) {
                            Ok(_) => {}
                            Err(error) => {
                                warn!(%error, mux_session_id, protocol, "mux udp upstream response encode failed");
                                break;
                            }
                        }
                    }
                    Err(error) => warn!(%error, mux_session_id, protocol, "mux udp upstream recv failed"),
                }
            }
            _ = wait_for_upstream_idle(socks5_idle) => {}
            Some(chain_result) = chain_tasks.join_next() => {
                match chain_result {
                    Ok(Ok((target, port, payload, session_id))) => {
                        last_activity = TokioInstant::now();
                        let response =
                            record_chain_udp_response_parts(proxy, target, port, payload, session_id);
                        match write_chain_response_sync(&response, || {
                            responder.write_response_for_target(
                                &response.target,
                                response.port,
                                &response.payload,
                            )
                        }) {
                            Ok(_) => {}
                            Err(error) => {
                                warn!(%error, mux_session_id, protocol, "mux udp chain response encode failed");
                                break;
                            }
                        }
                    }
                    Ok(Err(error)) => warn!(%error, mux_session_id, protocol, "mux udp chain response failed"),
                    Err(error) => warn!(%error, mux_session_id, protocol, "mux udp chain task panicked"),
                }
            }
        }
    }

    for completed in dispatch.finish_all() {
        log_completed_udp_flow(completed);
    }
    let _ = responder.end_inbound_stream();
}
