//! Shadowsocks UDP relay: protocol framing and routing through the UDP pipe.

use std::sync::Arc;

use shadowsocks::ShadowsocksInboundProfile;
use tokio::net::UdpSocket;
use tracing::warn;
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_flow::helpers::{
    record_chain_udp_response_received, record_direct_udp_response_parts,
};
use crate::runtime::Proxy;

impl Proxy {
    pub(crate) async fn ss_udp_relay_loop(
        &self,
        udp_socket: Arc<UdpSocket>,
        inbound_tag: &str,
        profile: ShadowsocksInboundProfile,
    ) -> Result<(), EngineError> {
        let mut dispatch = crate::runtime::udp_dispatch::UdpDispatch::new(inbound_tag).await?;
        let mut udp_session = profile.udp_session();
        let mut buf = [0u8; 65536];
        let mut direct_buf = [0u8; 65536];

        loop {
            let (direct_sock, chain_tasks) = dispatch.poll_sockets();

            tokio::select! {
                recv = udp_socket.recv_from(&mut buf) => {
                    let (n, client_addr) = match recv {
                        Ok(r) => r,
                        Err(e) => { warn!(error = %e, "ss udp recv error"); break Ok(()); }
                    };
                    let packet = &buf[..n];

                    let inbound_dispatch = match udp_session.decode_inbound_dispatch(packet) {
                        Ok(inbound_dispatch) => inbound_dispatch,
                        Err(_) => continue,
                    };

                    let sa = profile.inbound_auth();
                    match UdpPipe::new(self, &mut dispatch)
                        .dispatch(UdpPipeInput::from_inbound_dispatch(
                            &inbound_dispatch,
                            Some(&sa),
                        ))
                        .await
                    {
                        Ok(session_id) => {
                            udp_session.record_dispatch_success(
                                session_id,
                                inbound_dispatch.client_session_id(),
                                client_addr,
                            );
                        }
                        Err(error) => {
                            warn!(error = %error, "ss udp dispatch failed");
                        }
                    }
                }

                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    let response = record_direct_udp_response_parts(
                        self,
                        &dispatch,
                        sender,
                        &direct_buf[..n],
                    );
                    if let Ok(Some(written)) = udp_session
                        .send_response_for_target_proxy_session_to_client_tokio(
                            udp_socket.as_ref(),
                            response.accounting.session_id(),
                            &response.target,
                            response.port,
                            response.payload,
                        )
                        .await
                    {
                        response.accounting.record_sent(written);
                    }
                }

                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            let response_accounting = record_chain_udp_response_received(
                                self,
                                session_id,
                                payload.len(),
                            );
                            if let Ok(Some(written)) = udp_session
                                .send_response_for_target_proxy_session_to_client_tokio(
                                    udp_socket.as_ref(),
                                    session_id,
                                    &target,
                                    port,
                                    &payload,
                                )
                                .await
                            {
                                response_accounting.record_sent(written);
                            }
                        }
                        Ok(Err(error)) => {
                            warn!(error = %error, "ss chain response error");
                        }
                        Err(e) => {
                            warn!(error = %e, "ss chain task panicked");
                        }
                    }
                }
            }
        }
    }
}
