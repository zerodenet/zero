use async_trait::async_trait;
use tokio::select;
use tokio::time::Instant as TokioInstant;
use tracing::{info, warn};
use zero_core::{InboundUdpDispatch, Session, SessionAuth};
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

#[async_trait]
pub(crate) trait StreamUdpResponder<S>: Send
where
    S: Send,
{
    async fn read_inbound_dispatch(
        &mut self,
        client: &mut S,
    ) -> Result<Option<InboundUdpDispatch>, zero_core::Error>;

    async fn write_response_for_target(
        &mut self,
        client: &mut S,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error>;

    fn record_client_io(&mut self, _proxy: &Proxy, _client: &mut S) {}
}

pub(crate) struct StreamUdpRelayRequest<'a, S, R> {
    pub(crate) client: S,
    pub(crate) responder: R,
    pub(crate) session: &'a Session,
    pub(crate) inbound_tag: &'a str,
    pub(crate) protocol: &'static str,
    pub(crate) auth: Option<&'a SessionAuth>,
}

pub(crate) async fn run_stream_udp_relay<S, R>(
    proxy: &Proxy,
    request: StreamUdpRelayRequest<'_, S, R>,
) -> Result<(), EngineError>
where
    S: Send,
    R: StreamUdpResponder<S>,
{
    let StreamUdpRelayRequest {
        mut client,
        mut responder,
        session: _session,
        inbound_tag,
        protocol,
        auth,
    } = request;

    let mut dispatch = UdpDispatch::new(inbound_tag).await?;
    let mut last_activity = TokioInstant::now();
    let timeout = proxy.udp_upstream_idle_timeout();

    info!(
        inbound_tag = inbound_tag,
        protocol = protocol,
        "stream udp session started"
    );

    let mut direct_buf = vec![0_u8; 64 * 1024];
    let mut upstream_buf = vec![0_u8; 64 * 1024];

    loop {
        let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

        select! {
            _ = tokio::time::sleep_until(last_activity + timeout) => {
                info!(
                    inbound_tag = inbound_tag,
                    protocol = protocol,
                    "stream udp session idle timeout"
                );
                break;
            }
            read = responder.read_inbound_dispatch(&mut client) => {
                match read {
                    Ok(None) => break,
                    Ok(Some(inbound_dispatch)) => {
                        last_activity = TokioInstant::now();
                        responder.record_client_io(proxy, &mut client);
                        if let Err(error) = dispatch_inbound_udp_packet(
                            proxy,
                            &mut dispatch,
                            &inbound_dispatch,
                            auth,
                        )
                        .await
                        {
                            warn!(error = %error, protocol = protocol, "failed to process stream udp packet");
                        }
                    }
                    Err(error) => {
                        warn!(error = %error, protocol = protocol, "stream udp client read/decode error");
                        break;
                    }
                }
            }
            recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                let (n, sender) = recv?;
                last_activity = TokioInstant::now();
                let response = record_direct_udp_response_parts(
                    proxy,
                    &dispatch,
                    sender,
                    &direct_buf[..n],
                );
                write_direct_response(&response, || async {
                    responder
                        .write_response_for_target(
                            &mut client,
                            &response.target,
                            response.port,
                            response.payload,
                        )
                        .await
                })
                .await?;
                responder.record_client_io(proxy, &mut client);
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
                        write_upstream_response(&response, || async {
                            responder
                                .write_response_for_target(
                                    &mut client,
                                    &response.target,
                                    response.port,
                                    &response.payload,
                                )
                                .await
                        })
                        .await?;
                        responder.record_client_io(proxy, &mut client);
                    }
                    Err(error) => {
                        warn!(error = %error, protocol = protocol, "stream udp upstream recv error");
                    }
                }
            }
            _ = wait_for_upstream_idle(socks5_idle) => {}
            Some(chain_result) = chain_tasks.join_next() => {
                match chain_result {
                    Ok(Ok((target, port, payload, session_id))) => {
                        last_activity = TokioInstant::now();
                        let response =
                            record_chain_udp_response_parts(proxy, target, port, payload, session_id);
                        write_chain_response(&response, || async {
                            responder
                                .write_response_for_target(
                                    &mut client,
                                    &response.target,
                                    response.port,
                                    &response.payload,
                                )
                                .await
                        })
                        .await?;
                        responder.record_client_io(proxy, &mut client);
                    }
                    Ok(Err(error)) => warn!(error = %error, protocol = protocol, "stream udp chain response error"),
                    Err(error) => warn!(error = %error, protocol = protocol, "stream udp chain task panicked"),
                }
            }
        }
    }

    for completed in dispatch.finish_all() {
        log_completed_udp_flow(completed);
    }

    info!(
        inbound_tag = inbound_tag,
        protocol = protocol,
        "stream udp session ended"
    );

    Ok(())
}
