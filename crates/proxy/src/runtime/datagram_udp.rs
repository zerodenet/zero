use tokio::select;
use zero_core::{DatagramUdpResponder, InboundUdpDispatch, SessionAuth};
use zero_engine::EngineError;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    log_completed_udp_flow, record_chain_udp_response_parts, record_direct_udp_response_parts,
    record_upstream_udp_response_received, wait_for_upstream_idle,
};
use crate::runtime::udp_inbound_dispatch::dispatch_inbound_udp_packet;
use crate::runtime::udp_response::{
    write_optional_chain_response, write_optional_direct_response, write_optional_upstream_response,
};
use crate::runtime::Proxy;

type ChainUdpResponseResult = Result<
    Result<(zero_core::Address, u16, Vec<u8>, Option<u64>), EngineError>,
    tokio::task::JoinError,
>;

pub(crate) struct DatagramUdpRelayRequest<'a, S, R> {
    pub(crate) source: S,
    pub(crate) responder: R,
    pub(crate) inbound_tag: &'a str,
    pub(crate) poll_upstream: bool,
    pub(crate) auth: Option<SessionAuth>,
}

pub(crate) async fn run_datagram_udp_relay<S, R>(
    proxy: &Proxy,
    request: DatagramUdpRelayRequest<'_, S, R>,
) -> Result<(), EngineError>
where
    S: Send,
    R: DatagramUdpResponder<S>,
{
    let DatagramUdpRelayRequest {
        source,
        mut responder,
        inbound_tag,
        poll_upstream,
        auth,
    } = request;
    let mut dispatch = UdpDispatch::new(inbound_tag).await?;
    let mut direct_buf = vec![0_u8; 64 * 1024];
    let mut upstream_buf = vec![0_u8; 64 * 1024];

    loop {
        if poll_upstream {
            let (direct_sock, upstream_udp, upstream_idle_deadline, chain_tasks) =
                dispatch.poll_refs();
            select! {
                read = responder.read_inbound_dispatch(&source) => {
                    if !process_datagram_read(proxy, &mut dispatch, &mut responder, auth.as_ref(), read).await {
                        break;
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    let response =
                        record_direct_udp_response_parts(proxy, &dispatch, sender, &direct_buf[..n]);
                    let _ = write_optional_direct_response(&response, || async {
                        responder
                            .write_response_for_session(
                                &source,
                                response.accounting.session_id(),
                                &response.target,
                                response.port,
                                response.payload,
                            )
                            .await
                    })
                    .await;
                }
                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            let response = record_upstream_udp_response_received(
                                proxy,
                                &mut dispatch,
                                proxy.udp_upstream_idle_timeout(),
                                pkt,
                            );
                            let _ = write_optional_upstream_response(&response, || async {
                                responder
                                    .write_response_for_session(
                                        &source,
                                        response.accounting.session_id(),
                                        &response.target,
                                        response.port,
                                        &response.payload,
                                    )
                                    .await
                            })
                            .await;
                        }
                        Err(error) => tracing::warn!(error = %error, "datagram udp upstream response error"),
                    }
                }
                _ = wait_for_upstream_idle(upstream_idle_deadline) => {}
                Some(chain_result) = chain_tasks.join_next() => {
                    handle_chain_result(proxy, &source, &mut responder, chain_result).await;
                }
            }
        } else {
            let (direct_sock, chain_tasks) = dispatch.poll_sockets();
            select! {
                read = responder.read_inbound_dispatch(&source) => {
                    if !process_datagram_read(proxy, &mut dispatch, &mut responder, auth.as_ref(), read).await {
                        break;
                    }
                }
                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    let response =
                        record_direct_udp_response_parts(proxy, &dispatch, sender, &direct_buf[..n]);
                    let _ = write_optional_direct_response(&response, || async {
                        responder
                            .write_response_for_session(
                                &source,
                                response.accounting.session_id(),
                                &response.target,
                                response.port,
                                response.payload,
                            )
                            .await
                    })
                    .await;
                }
                Some(chain_result) = chain_tasks.join_next() => {
                    handle_chain_result(proxy, &source, &mut responder, chain_result).await;
                }
            }
        }
    }

    for completed in dispatch.finish_all() {
        log_completed_udp_flow(completed);
    }

    Ok(())
}

async fn process_datagram_read<S, R>(
    proxy: &Proxy,
    dispatch: &mut UdpDispatch,
    responder: &mut R,
    request_auth: Option<&SessionAuth>,
    read: Result<Option<InboundUdpDispatch>, zero_core::Error>,
) -> bool
where
    S: Send,
    R: DatagramUdpResponder<S>,
{
    let inbound_dispatch = match read {
        Ok(Some(inbound_dispatch)) => inbound_dispatch,
        Ok(None) => return false,
        Err(error) => {
            tracing::warn!(error = %error, "datagram udp inbound read/decode error");
            return false;
        }
    };
    let auth = request_auth.or_else(|| responder.auth());
    match dispatch_inbound_udp_packet(proxy, dispatch, &inbound_dispatch, auth).await {
        Ok(session_id) => responder.on_dispatch_success(session_id, &inbound_dispatch),
        Err(error) => tracing::warn!(error = %error, "datagram udp dispatch failed"),
    }
    true
}

async fn handle_chain_result<S, R>(
    proxy: &Proxy,
    source: &S,
    responder: &mut R,
    chain_result: ChainUdpResponseResult,
) where
    S: Send,
    R: DatagramUdpResponder<S>,
{
    match chain_result {
        Ok(Ok((target, port, payload, session_id))) => {
            let response =
                record_chain_udp_response_parts(proxy, target, port, payload, session_id);
            let _ = write_optional_chain_response(&response, || async {
                responder
                    .write_response_for_session(
                        source,
                        session_id,
                        &response.target,
                        response.port,
                        &response.payload,
                    )
                    .await
            })
            .await;
        }
        Ok(Err(error)) => tracing::warn!(error = %error, "datagram udp chain response error"),
        Err(error) => tracing::warn!(error = %error, "datagram udp chain task panicked"),
    }
}
