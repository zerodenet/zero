//! Shadowsocks UDP relay: protocol framing and routing through the UDP pipe.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use shadowsocks::{CipherKind, ShadowsocksDatagramCodec};
use tokio::net::UdpSocket;
use tracing::warn;
use zero_core::{Address, ProtocolType};
use zero_engine::EngineError;
use zero_traits::DatagramCodec;

use crate::runtime::pipe::{KernelPipe, UdpPipe, UdpPipeInput};
use crate::runtime::udp_flow::helpers::address_from_socket_addr;
use crate::runtime::Proxy;

impl Proxy {
    pub(crate) async fn ss_udp_relay_loop(
        &self,
        udp_socket: Arc<UdpSocket>,
        inbound_tag: &str,
        password: &str,
        cipher: CipherKind,
    ) -> Result<(), EngineError> {
        let mut dispatch = crate::runtime::udp_dispatch::UdpDispatch::new(inbound_tag).await?;
        // Map session_id -> client_addr for response delivery.
        let mut client_sessions: HashMap<u64, SocketAddr> = HashMap::new();
        // For 2022 (blake3): map internal dispatch session_id -> the client's
        // SIP022 session id, so server-to-client responses can echo it.
        let mut client_ss_session_ids: HashMap<u64, u64> = HashMap::new();
        // SIP022 3.2.4: per-client-session sliding-window replay filter over
        // packet ids, keyed by the client SIP022 session id.
        let mut udp_replay_windows: HashMap<u64, shadowsocks::ReplayWindow> = HashMap::new();

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

                    // Decode the client datagram. For 2022 (blake3) also recover
                    // the client SIP022 session id + packet id.
                    let (target, port, payload, client_ss_sid, client_ss_pid) = if cipher
                        .is_blake3()
                    {
                        match shadowsocks::decode_udp_datagram_2022_session(
                            cipher,
                            password.as_bytes(),
                            packet,
                        ) {
                            Ok((t, p, pl, sid, pid)) => (t, p, pl, sid, pid),
                            Err(_) => continue,
                        }
                    } else {
                        let codec = ShadowsocksDatagramCodec {
                            cipher,
                            password: password.as_bytes().to_vec(),
                        };
                        match <ShadowsocksDatagramCodec as DatagramCodec<Address>>::decode(
                            &codec, packet,
                        ) {
                            Some((t, p, pl)) => (t, p, pl, 0u64, 0u64),
                            None => continue,
                        }
                    };

                    // SIP022 3.2.4: reject duplicate or out-of-window packet
                    // ids per client session (sliding-window replay filter).
                    if cipher.is_blake3()
                        && !udp_replay_windows
                            .entry(client_ss_sid)
                            .or_default()
                            .check_and_update(client_ss_pid)
                    {
                        continue;
                    }

                    let mut sa = zero_core::SessionAuth::new("shadowsocks");
                    sa.principal_key = Some(password.to_owned());
                    let flow_isolation_key = if cipher.is_blake3() {
                        Some(client_ss_sid)
                    } else {
                        None
                    };
                    match UdpPipe::new(self, &mut dispatch)
                        .dispatch(UdpPipeInput {
                            target,
                            port,
                            payload: &payload,
                            protocol: ProtocolType::Shadowsocks,
                            auth: Some(&sa),
                            client_session_id: flow_isolation_key,
                        })
                        .await
                    {
                        Ok(session_id) => {
                            client_sessions.insert(session_id, client_addr);
                            if cipher.is_blake3() {
                                client_ss_session_ids.insert(session_id, client_ss_sid);
                            }
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
                            ss_send_encrypted(SsEncryptedResponse {
                                socket: udp_socket.as_ref(),
                                cipher,
                                password,
                                client_session_id: client_ss_session_ids.get(&sid).copied(),
                                target: &address_from_socket_addr(sender),
                                port: sender.port(),
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
                                    ss_send_encrypted(SsEncryptedResponse {
                                        socket: udp_socket.as_ref(),
                                        cipher,
                                        password,
                                        client_session_id: client_ss_session_ids.get(&sid).copied(),
                                        target: &target,
                                        port,
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
struct SsEncryptedResponse<'a> {
    socket: &'a UdpSocket,
    cipher: CipherKind,
    password: &'a str,
    client_session_id: Option<u64>,
    target: &'a Address,
    port: u16,
    payload: &'a [u8],
    client: SocketAddr,
}

async fn ss_send_encrypted(response: SsEncryptedResponse<'_>) {
    let resp = if response.cipher.is_blake3() {
        shadowsocks::encode_udp_response_2022(
            response.cipher,
            response.password.as_bytes(),
            response.client_session_id.unwrap_or(0),
            response.target,
            response.port,
            response.payload,
        )
    } else {
        legacy_ss_udp_encode(
            response.cipher,
            response.password,
            response.target,
            response.port,
            response.payload,
        )
    };
    let Ok(resp) = resp else {
        return;
    };
    let _ = response.socket.send_to(&resp, response.client).await;
}

/// Encode a legacy (non-2022) Shadowsocks UDP datagram.
fn legacy_ss_udp_encode(
    cipher: CipherKind,
    password: &str,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, zero_core::Error> {
    let codec = ShadowsocksDatagramCodec {
        cipher,
        password: password.as_bytes().to_vec(),
    };
    <ShadowsocksDatagramCodec as DatagramCodec<Address>>::encode(&codec, target, port, payload)
}
