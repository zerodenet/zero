use std::sync::Arc;

use tokio::select;
use tracing::warn;
use zero_engine::EngineError;

use crate::inbound::udp_dispatch::dispatch_inbound_udp_packet;
use crate::inbound::udp_response::{
    write_optional_chain_response_sync, write_optional_direct_response_sync,
    write_optional_upstream_response_sync,
};
use crate::runtime::udp_flow::helpers::{
    record_chain_udp_response_parts, record_direct_udp_response_parts,
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
        let mut udp_responder = hysteria2::Hysteria2Inbound.udp_responder();

        let mut direct_buf = [0u8; 65536];
        let mut upstream_buf = [0u8; 65536];

        loop {
            let (direct_sock, upstream_udp, socks5_idle, chain_tasks) = dispatch.poll_refs();

            select! {
                dg = udp_responder.read_inbound_dispatch_from_datagram(&conn) => {
                    match dg {
                        Ok(tracked) => {
                            let _ = dispatch_inbound_udp_packet(
                                    &proxy,
                                    &mut dispatch,
                                    tracked.dispatch(),
                                    None,
                                )
                                .await
                                .inspect(|sid| {
                                    udp_responder.record_dispatch_success(*sid, &tracked);
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
                    let response = record_direct_udp_response_parts(
                        &proxy,
                        &dispatch,
                        sender,
                        &direct_buf[..n],
                    );
                    let _ = write_optional_direct_response_sync(&response, || {
                        udp_responder.send_response_for_target_proxy_session(
                            &conn,
                            response.accounting.session_id(),
                            &response.target,
                            response.port,
                            response.payload,
                        )
                    });
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
                            let _ = write_optional_upstream_response_sync(&response, || {
                                udp_responder.send_response_for_target_proxy_session(
                                    &conn,
                                    response.accounting.session_id(),
                                    &response.target,
                                    response.port,
                                    &response.payload,
                                )
                            });
                        }
                        Err(error) => warn!(error = %error, "h2 upstream response error"),
                    }
                }

                _ = wait_for_upstream_idle(socks5_idle) => {}

                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            let response =
                                record_chain_udp_response_parts(&proxy, target, port, payload, session_id);
                            let _ = write_optional_chain_response_sync(&response, || {
                                udp_responder.send_response_for_target_proxy_session(
                                    &conn,
                                    session_id,
                                    &response.target,
                                    response.port,
                                    &response.payload,
                                )
                            });
                        }
                        Ok(Err(error)) => warn!(error = %error, "h2 chain response error"),
                        Err(e) => warn!(error = %e, "h2 chain task panicked"),
                    }
                }
            }
        }
    }
}
