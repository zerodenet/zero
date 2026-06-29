//! Shadowsocks UDP relay: protocol framing and routing through the UDP pipe.

use std::sync::Arc;

use shadowsocks::ShadowsocksInboundProfile;
use tokio::net::UdpSocket;
use tracing::warn;
use zero_core::ProtocolType;
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
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

                    let dispatch_parts = match udp_session.decode_dispatch_parts(packet) {
                        Ok(dispatch_parts) => dispatch_parts,
                        Err(_) => continue,
                    };

                    let sa = profile.inbound_auth();
                    let (target, port, payload, client_session_id) = dispatch_parts.pipe_parts();
                    match UdpPipe::new(self, &mut dispatch)
                        .dispatch(UdpPipeInput {
                            target: target.clone(),
                            port,
                            payload,
                            protocol: ProtocolType::Shadowsocks,
                            auth: Some(&sa),
                            client_session_id,
                        })
                        .await
                    {
                        Ok(session_id) => {
                            udp_session.record_dispatch_success(
                                session_id,
                                &dispatch_parts,
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
                    if let Some(sid) = dispatch.direct_response_session_id(sender) {
                        let _ = udp_session
                            .send_proxy_session_response_to_sender_tokio(
                                udp_socket.as_ref(),
                                sid,
                                sender,
                                &direct_buf[..n],
                            )
                            .await;
                    }
                }

                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            if let Some(sid) = session_id {
                                let _ = udp_session
                                    .send_proxy_session_response_to_client_tokio(
                                        udp_socket.as_ref(),
                                        sid,
                                        &target,
                                        port,
                                        &payload,
                                    )
                                    .await;
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
