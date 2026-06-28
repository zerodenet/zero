//! Shadowsocks UDP relay: protocol framing and routing through the UDP pipe.

use std::net::SocketAddr;
use std::sync::Arc;

use std::collections::HashMap;

use shadowsocks::udp::ShadowsocksInboundUdpSession;
use shadowsocks::ShadowsocksInboundProfile;
use tokio::net::UdpSocket;
use tracing::warn;
use zero_core::ProtocolType;
use zero_engine::EngineError;

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_flow::helpers::address_from_socket_addr;
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
        // Map session_id -> client_addr for response delivery.
        let mut client_sessions: HashMap<u64, SocketAddr> = HashMap::new();
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

                    let request = match udp_session.decode_request(packet) {
                        Ok(request) => request,
                        Err(_) => continue,
                    };

                    let mut sa = zero_core::SessionAuth::new("shadowsocks");
                    sa.principal_key = Some(profile.principal_key());
                    let shadowsocks::udp::ShadowsocksInboundUdpDispatchParts {
                        target,
                        port,
                        payload,
                        client_session_id,
                    } = request.into_dispatch_parts();
                    match UdpPipe::new(self, &mut dispatch)
                        .dispatch(UdpPipeInput {
                            target,
                            port,
                            payload: &payload,
                            protocol: ProtocolType::Shadowsocks,
                            auth: Some(&sa),
                            client_session_id,
                        })
                        .await
                    {
                        Ok(session_id) => {
                            client_sessions.insert(session_id, client_addr);
                            udp_session.record_proxy_session(session_id, client_session_id);
                        }
                        Err(error) => {
                            warn!(error = %error, "ss udp dispatch failed");
                        }
                    }
                }

                recv = direct_sock.recv_from_addr(&mut direct_buf) => {
                    let (n, sender) = recv?;
                    if let Some(sid) = dispatch.direct_response_session_id(sender) {
                        if let Some(&client) = client_sessions.get(&sid) {
                            ss_send_protocol_response(SsProtocolResponse {
                                socket: udp_socket.as_ref(),
                                udp_session: &udp_session,
                                response_target: udp_session.response_target_for_proxy_session(
                                    sid,
                                    &address_from_socket_addr(sender),
                                    sender.port(),
                                ),
                                payload: &direct_buf[..n],
                                client,
                            })
                            .await;
                        }
                    }
                }

                Some(chain_result) = chain_tasks.join_next() => {
                    match chain_result {
                        Ok(Ok((target, port, payload, session_id))) => {
                            if let Some(sid) = session_id {
                                if let Some(&client) = client_sessions.get(&sid) {
                                    ss_send_protocol_response(SsProtocolResponse {
                                        socket: udp_socket.as_ref(),
                                        udp_session: &udp_session,
                                        response_target: udp_session
                                            .response_target_for_proxy_session(sid, &target, port),
                                        payload: &payload,
                                        client,
                                    })
                                    .await;
                                }
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

/// Encode and send one Shadowsocks UDP response datagram.
///
/// For 2022 (blake3) ciphers this produces a server-to-client response that
/// echoes `client_session_id` (SIP022 3.2.3); for legacy AEAD it produces the
/// stateless datagram via the shared codec.
struct SsProtocolResponse<'a> {
    socket: &'a UdpSocket,
    udp_session: &'a ShadowsocksInboundUdpSession,
    response_target: shadowsocks::udp::ShadowsocksInboundUdpResponseTarget,
    payload: &'a [u8],
    client: SocketAddr,
}

async fn ss_send_protocol_response(response: SsProtocolResponse<'_>) {
    let resp = response
        .udp_session
        .response_frame(&response.response_target, response.payload);
    let Ok(response_datagram) = resp else {
        return;
    };
    let _ = response
        .socket
        .send_to(response_datagram.datagram(), response.client)
        .await;
}
