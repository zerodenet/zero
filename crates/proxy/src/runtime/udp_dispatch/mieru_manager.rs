use tokio::task::JoinSet;
use zero_core::{Address, Session};
use zero_engine::EngineError;

use super::{ChainTask, FlowFailure};
use crate::runtime::Proxy;

#[cfg(feature = "mieru")]
use {
    super::packet_path_traits::{MieruUdpPeer, UdpFlowContext, UdpPacketRef, UdpPeerEndpoint},
    crate::transport::TcpRelayStream,
    mieru::{MieruOutbound, MieruProtocol, MieruUdpAssociatePacket},
    std::collections::HashMap,
    std::sync::Arc,
    tokio::io::{AsyncReadExt, AsyncWriteExt},
    tokio::sync::{broadcast, mpsc, Mutex},
    zero_traits::UdpPacketFraming,
};

#[cfg(feature = "mieru")]
type RecvItem = (Address, u16, Vec<u8>);

#[cfg(feature = "mieru")]
pub(crate) struct MieruChainManager {
    upstreams: HashMap<MieruKey, MieruEntry>,
}

#[cfg(feature = "mieru")]
struct MieruEntry {
    send_tx: mpsc::Sender<Vec<u8>>,
    recv_tx: broadcast::Sender<RecvItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg(feature = "mieru")]
enum MieruKey {
    Leaf {
        server: String,
        port: u16,
        username: String,
        password: String,
    },
    Relay {
        session_id: u64,
    },
}

#[cfg(feature = "mieru")]
impl MieruChainManager {
    pub(super) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        _session: &Session,
        peer: MieruUdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let session_id = ctx.session_id;
        let key = if peer.relay_chain {
            MieruKey::Relay { session_id }
        } else {
            MieruKey::Leaf {
                server: peer.endpoint.server.to_owned(),
                port: peer.endpoint.port,
                username: peer.username.to_owned(),
                password: peer.password.to_owned(),
            }
        };

        if let Some(entry) = self.upstreams.get(&key) {
            Self::spawn_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
            let wrapped = Self::packet(packet_ref.target, packet_ref.port, packet_ref.payload)
                .map_err(|error| FlowFailure {
                    stage: "mieru_udp_packet",
                    error,
                    upstream: Some(peer.endpoint.upstream()),
                })?;
            let _ = entry.send_tx.send(wrapped).await;
            return Ok(sent);
        }

        if peer.relay_chain {
            return Err(FlowFailure {
                stage: "mieru_relay_upstream",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "mieru relay upstream is not established",
                )),
                upstream: Some(peer.endpoint.upstream()),
            });
        }

        let entry = Self::establish_direct(proxy, &peer)
            .await
            .map_err(|e| FlowFailure {
                stage: "mieru_establish",
                error: e,
                upstream: Some(peer.endpoint.upstream()),
            })?;

        Self::spawn_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
        let send_tx = entry.send_tx.clone();
        self.upstreams.insert(key, entry);

        let wrapped = Self::packet(packet_ref.target, packet_ref.port, packet_ref.payload)
            .map_err(|error| FlowFailure {
                stage: "mieru_udp_packet",
                error,
                upstream: Some(peer.endpoint.upstream()),
            })?;
        let _ = send_tx.send(wrapped).await;
        Ok(sent)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn send_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        session_id: u64,
        proxy: &Proxy,
        session: &Session,
        server: &str,
        port: u16,
        username: &str,
        password: &str,
        relay_chain: bool,
        target: &Address,
        target_port: u16,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.send(
            UdpFlowContext {
                chain_tasks,
                session_id,
            },
            proxy,
            session,
            MieruUdpPeer {
                endpoint: UdpPeerEndpoint { server, port },
                username,
                password,
                relay_chain,
            },
            UdpPacketRef {
                target,
                port: target_port,
                payload,
            },
        )
        .await
    }

    async fn send_relay(
        &mut self,
        ctx: UdpFlowContext<'_>,
        stream: TcpRelayStream,
        peer: MieruUdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let session_id = ctx.session_id;
        let key = MieruKey::Relay { session_id };
        let entry = Self::establish_packet_stream(stream, peer.username, peer.password)
            .await
            .map_err(|e| FlowFailure {
                stage: "mieru_relay_establish",
                error: e,
                upstream: Some(peer.endpoint.upstream()),
            })?;

        Self::spawn_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
        let send_tx = entry.send_tx.clone();
        self.upstreams.insert(key, entry);

        let wrapped = Self::packet(packet_ref.target, packet_ref.port, packet_ref.payload)
            .map_err(|error| FlowFailure {
                stage: "mieru_udp_packet",
                error,
                upstream: Some(peer.endpoint.upstream()),
            })?;
        let _ = send_tx.send(wrapped).await;
        Ok(packet_ref.payload.len())
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn send_relay_existing(
        &mut self,
        chain_tasks: &mut JoinSet<ChainTask>,
        session_id: u64,
        stream: TcpRelayStream,
        server: &str,
        port: u16,
        username: &str,
        password: &str,
        target: &Address,
        target_port: u16,
        payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        self.send_relay(
            UdpFlowContext {
                chain_tasks,
                session_id,
            },
            stream,
            MieruUdpPeer {
                endpoint: UdpPeerEndpoint { server, port },
                username,
                password,
                relay_chain: true,
            },
            UdpPacketRef {
                target,
                port: target_port,
                payload,
            },
        )
        .await
    }

    async fn establish_direct(
        proxy: &Proxy,
        peer: &MieruUdpPeer<'_>,
    ) -> Result<MieruEntry, EngineError> {
        let socket = proxy
            .protocols
            .direct_connector()
            .connect_host(
                peer.endpoint.server,
                peer.endpoint.port,
                proxy.resolver.as_ref(),
            )
            .await?;

        Self::establish_packet_stream(TcpRelayStream::new(socket), peer.username, peer.password)
            .await
    }

    async fn establish_packet_stream(
        mut stream: TcpRelayStream,
        username: &str,
        password: &str,
    ) -> Result<MieruEntry, EngineError> {
        // Establish the encrypted mieru session, then negotiate socks5 UDP
        // ASSOCIATE inside the tunnel (CMD=3). mieru conveys the UDP target via
        // socks5, and the session must complete the UDP ASSOCIATE handshake
        // before UDP packets can flow.
        let mut outbound = MieruOutbound::connect(&mut stream, username, password)
            .await
            .map_err(|e| {
                EngineError::Io(std::io::Error::other(format!("mieru udp handshake: {e}")))
            })?;

        // Send socks5 UDP ASSOCIATE request: [VER, CMD=3, RSV, ATYP=IPv4, 0.0.0.0:0].
        let assoc_req = [0x05u8, 0x03, 0x00, 0x01, 0, 0, 0, 0, 0, 0];
        let assoc_seg = outbound.encrypt_client_data(&assoc_req).map_err(|e| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp assoc encrypt: {e}"
            )))
        })?;
        stream.write_all(&assoc_seg).await.map_err(|e| {
            EngineError::Io(std::io::Error::other(format!("mieru udp assoc write: {e}")))
        })?;
        stream.flush().await.map_err(|e| {
            EngineError::Io(std::io::Error::other(format!("mieru udp assoc flush: {e}")))
        })?;

        // Read the UDP ASSOCIATE response (one data segment) and check REP == 0.
        let mut assoc_raw = Vec::new();
        let assoc_resp = loop {
            match outbound.decrypt_server_data_with_consumed(&assoc_raw) {
                Ok((segment, consumed)) => {
                    assoc_raw.drain(..consumed);
                    break segment.payload;
                }
                Err(zero_core::Error::Protocol("mieru: need more data")) => {
                    let mut scratch = [0u8; 4096];
                    let n = stream.read(&mut scratch).await.map_err(|e| {
                        EngineError::Io(std::io::Error::other(format!("mieru udp assoc read: {e}")))
                    })?;
                    if n == 0 {
                        return Err(EngineError::Io(std::io::Error::other(
                            "mieru udp assoc: connection closed",
                        )));
                    }
                    assoc_raw.extend_from_slice(&scratch[..n]);
                }
                Err(e) => {
                    return Err(EngineError::Io(std::io::Error::other(format!(
                        "mieru udp assoc decrypt: {e}"
                    ))))
                }
            }
        };
        if assoc_resp.len() < 4 || assoc_resp[0] != 0x05 || assoc_resp[1] != 0x00 {
            return Err(EngineError::Io(std::io::Error::other(format!(
                "mieru udp assoc rejected: {:?}",
                &assoc_resp[..assoc_resp.len().min(4)]
            ))));
        }

        let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);
        let (recv_tx, _) = broadcast::channel::<RecvItem>(32);

        let shared_outbound = Arc::new(Mutex::new(outbound));
        let (mut read_half, mut write_half) = tokio::io::split(stream);

        let send_outbound = shared_outbound.clone();
        tokio::spawn(async move {
            while let Some(payload) = send_rx.recv().await {
                let encrypted = {
                    let mut ob = send_outbound.lock().await;
                    match ob.encrypt_client_data(&payload) {
                        Ok(encrypted) => encrypted,
                        Err(_) => break,
                    }
                };
                if write_half.write_all(&encrypted).await.is_err() {
                    break;
                }
                if write_half.flush().await.is_err() {
                    break;
                }
            }
        });

        let recv_outbound = shared_outbound.clone();
        let recv_tx2 = recv_tx.clone();
        tokio::spawn(async move {
            let mut raw = Vec::new();
            loop {
                let mut scratch = [0u8; 4096];
                match read_half.read(&mut scratch).await {
                    Ok(0) => break,
                    Ok(n) => raw.extend_from_slice(&scratch[..n]),
                    Err(_) => break,
                }
                loop {
                    let decrypted = {
                        let mut ob = recv_outbound.lock().await;
                        ob.decrypt_server_data_with_consumed(&raw)
                    };
                    match decrypted {
                        Ok((segment, consumed)) => {
                            raw.drain(..consumed);
                            if !segment.payload.is_empty() {
                                if let Ok(unwrapped) =
                                    Self::decode_associate_packet(&segment.payload)
                                {
                                    if let Ok(packet) = socks5::parse_udp_packet(&unwrapped.payload)
                                    {
                                        if recv_tx2
                                            .send((packet.target, packet.port, packet.payload))
                                            .is_err()
                                        {
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) if e == zero_core::Error::Protocol("mieru: need more data") => break,
                        Err(_) => return,
                    }
                }
            }
        });

        Ok(MieruEntry { send_tx, recv_tx })
    }

    fn spawn_bridge(
        chain_tasks: &mut JoinSet<ChainTask>,
        recv_tx: broadcast::Sender<RecvItem>,
        session_id: u64,
    ) {
        let mut recv_rx = recv_tx.subscribe();
        chain_tasks.spawn(async move {
            let (target, port, payload) = recv_rx
                .recv()
                .await
                .map_err(|_| EngineError::Io(std::io::Error::other("mieru upstream closed")))?;
            Ok((target, port, payload, Some(session_id)))
        });
    }

    fn packet(target: &Address, target_port: u16, payload: &[u8]) -> Result<Vec<u8>, EngineError> {
        let packet =
            socks5::build_udp_packet(target, target_port, payload).map_err(EngineError::from)?;
        Self::encode_associate_packet(&packet)
    }

    fn encode_associate_packet(payload: &[u8]) -> Result<Vec<u8>, EngineError> {
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::encode_udp_packet(
            &MieruProtocol,
            &MieruUdpAssociatePacket { payload },
        )
        .map_err(EngineError::from)
    }

    fn decode_associate_packet(
        payload: &[u8],
    ) -> Result<mieru::MieruUdpAssociatePayload, EngineError> {
        <MieruProtocol as UdpPacketFraming<MieruUdpAssociatePacket<'_>>>::decode_udp_packet(
            &MieruProtocol,
            payload,
        )
        .map_err(EngineError::from)
    }
}

#[cfg(not(feature = "mieru"))]
pub(crate) struct MieruChainManager;

#[cfg(not(feature = "mieru"))]
impl MieruChainManager {
    pub(super) fn new() -> Self {
        Self
    }

    #[allow(unused_variables, clippy::too_many_arguments)]
    pub(crate) async fn send_existing(
        &mut self,
        _chain_tasks: &mut JoinSet<ChainTask>,
        _session_id: u64,
        _proxy: &Proxy,
        _session: &Session,
        _server: &str,
        _port: u16,
        _username: &str,
        _password: &str,
        _relay_chain: bool,
        _target: &Address,
        _target_port: u16,
        _payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        Err(FlowFailure {
            stage: "mieru_feature",
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Mieru requires feature `mieru`",
            )),
            upstream: None,
        })
    }
    #[allow(unused_variables, clippy::too_many_arguments)]
    pub(crate) async fn send_relay_existing(
        &mut self,
        _stream: crate::transport::TcpRelayStream,
        _server: &str,
        _port: u16,
        _username: &str,
        _password: &str,
        _target: &Address,
        _target_port: u16,
        _payload: &[u8],
    ) -> Result<usize, FlowFailure> {
        Err(FlowFailure {
            stage: "mieru_feature",
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Mieru requires feature `mieru`",
            )),
            upstream: None,
        })
    }
}
