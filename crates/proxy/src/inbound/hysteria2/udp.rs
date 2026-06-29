use std::sync::Arc;

use tokio::select;
use tracing::warn;
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_flow::helpers::{
    record_chain_udp_response_received, record_direct_udp_response_received,
    record_upstream_udp_response_received, wait_for_upstream_idle,
};
use crate::runtime::Proxy;

impl Proxy {
    pub(super) async fn hysteria2_datagram_loop(
        conn: Arc<quinn::Connection>,
        inbound_tag: String,
        proxy: Proxy,
    ) -> Result<(), EngineError> {
        let mut dispatch = crate::runtime::udp_dispatch::UdpDispatch::new(&inbound_tag).await?;
        let mut udp_session = hysteria2::Hysteria2Inbound.udp_session();

        let mut direct_buf = [0u8; 65536];
        let mut upstream_buf = [0u8; 65536];

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

            select! {
                dg = udp_session.read_inbound_dispatch_from_datagram(&conn) => {
                    match dg {
                        Ok(tracked) => {
                            let _ = UdpPipe::new(&proxy, &mut dispatch)
                                .dispatch(UdpPipeInput::from_inbound_dispatch(
                                    tracked.dispatch(),
                                    None,
                                ))
                                .await
                                .inspect(|sid| {
                                    udp_session.record_dispatch_success(*sid, &tracked);
                                })
                                .inspect_err(|e| {
                                    warn!(error = %e, "h2 udp dispatch failed");
                                });
                        }
                        Err(e) => {
                            warn!(error = %e, "hysteria2 datagram read/decode error");
                            break Ok(());
                        }
                    }
                }

                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    let response_accounting =
                        record_direct_udp_response_received(&proxy, &dispatch, sender, n);
                    if let Ok(Some(written)) = udp_session.send_response_to_socket_addr_for_proxy_session(
                        &conn,
                        response_accounting.session_id(),
                        sender,
                        &direct_buf[..n],
                    ) {
                        response_accounting.record_sent(written);
                    }
                }

                upstream = upstream_udp.recv_response(&mut upstream_buf) => {
                    match upstream {
                        Ok(pkt) => {
                            let response = record_upstream_udp_response_received(
                                &proxy,
                                &mut dispatch,
                                proxy.udp_upstream_idle_timeout(),
                                pkt,
                            );
                            let client_response =
                                hysteria2::udp::Hysteria2InboundUdpClientResponse::new(
                                    &response.target,
                                    response.port,
                                    &response.payload,
                                );
                            if let Ok(Some(written)) = udp_session.send_client_response_for_proxy_session(
                                &conn,
                                response.accounting.session_id(),
                                client_response,
                            ) {
                                response.accounting.record_sent(written);
                            }
                        }
                        Err(error) => warn!(error = %error, "h2 upstream response error"),
                    }
                }

                _ = wait_for_upstream_idle(socks5_idle) => {}

                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            let client_response =
                                hysteria2::udp::Hysteria2InboundUdpClientResponse::new(
                                    &target,
                                    port,
                                    &payload,
                                );
                            let response_accounting =
                                record_chain_udp_response_received(
                                    &proxy,
                                    session_id,
                                    client_response.payload_len(),
                                );
                            if let Ok(Some(written)) = udp_session.send_client_response_for_proxy_session(
                                &conn,
                                session_id,
                                client_response,
                            ) {
                                response_accounting.record_sent(written);
                            }
                        }
                        Ok(Err(error)) => warn!(error = %error, "h2 chain response error"),
                        Err(e) => warn!(error = %e, "h2 chain task panicked"),
                    }
                }
            }
        }
    }
}
