#[cfg(feature = "trojan")]
use {
    super::ChainTask,
    crate::transport::{MeteredStream, TcpRelayStream},
    std::collections::HashMap,
    std::io,
    tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    tokio::sync::{broadcast, mpsc},
    tokio::task::JoinSet,
    trojan::{TrojanOutbound, TrojanUdpPacket, TrojanUdpPacketTunnelTarget},
    zero_core::Address,
    zero_traits::{AsyncSocket, UdpPacketStreamFraming, UdpPacketTunnelProtocol},
};

use zero_core::Session;
use zero_engine::EngineError;

use super::{FlowFailure, TrojanUdpPeer, UdpFlowContext, UdpPacketRef};
use crate::runtime::Proxy;

#[cfg(feature = "trojan")]
pub(super) struct TrojanChainManager {
    upstreams: HashMap<TrojanKey, TrojanEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg(feature = "trojan")]
enum TrojanKey {
    Leaf {
        server: String,
        port: u16,
        password: String,
    },
    Relay {
        session_id: u64,
    },
}

#[cfg(feature = "trojan")]
struct TrojanEntry {
    send_tx: mpsc::Sender<TrojanUdpPacket>,
    recv_tx: broadcast::Sender<TrojanUdpPacket>,
}

#[cfg(feature = "trojan")]
impl TrojanChainManager {
    pub(super) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub(super) async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        session: &Session,
        peer: TrojanUdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let sent = packet_ref.payload.len();
        let session_id = ctx.session_id;
        let key = if peer.relay_chain {
            TrojanKey::Relay { session_id }
        } else {
            TrojanKey::Leaf {
                server: peer.endpoint.server.to_owned(),
                port: peer.endpoint.port,
                password: peer.password.to_owned(),
            }
        };

        if let Some(entry) = self.upstreams.get(&key) {
            Self::spawn_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
            let _ = entry
                .send_tx
                .send(Self::packet(
                    packet_ref.target,
                    packet_ref.port,
                    packet_ref.payload,
                ))
                .await;
            return Ok(sent);
        }

        if peer.relay_chain {
            return Err(FlowFailure {
                stage: "trojan_relay_upstream",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::NotConnected,
                    "trojan relay upstream is not established",
                )),
                upstream: Some(peer.endpoint.upstream()),
            });
        }

        let entry =
            Self::establish_direct(proxy, session, &peer, packet_ref.target, packet_ref.port)
                .await
                .map_err(|e| FlowFailure {
                    stage: "trojan_establish",
                    error: e,
                    upstream: Some(peer.endpoint.upstream()),
                })?;

        Self::spawn_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
        let send_tx = entry.send_tx.clone();
        self.upstreams.insert(key, entry);

        let _ = send_tx
            .send(Self::packet(
                packet_ref.target,
                packet_ref.port,
                packet_ref.payload,
            ))
            .await;

        Ok(sent)
    }

    pub(super) async fn send_relay(
        &mut self,
        ctx: UdpFlowContext<'_>,
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        proxy: &Proxy,
        session: &Session,
        peer: TrojanUdpPeer<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let session_id = ctx.session_id;
        let key = TrojanKey::Relay { session_id };
        let entry = Self::establish_over_relay_stream(
            stream,
            tls_server_name,
            proxy,
            session,
            &peer,
            packet_ref.target,
            packet_ref.port,
        )
        .await
        .map_err(|e| FlowFailure {
            stage: "trojan_relay_establish",
            error: e,
            upstream: Some(peer.endpoint.upstream()),
        })?;

        Self::spawn_bridge(ctx.chain_tasks, entry.recv_tx.clone(), session_id);
        let send_tx = entry.send_tx.clone();
        self.upstreams.insert(key, entry);
        let _ = send_tx
            .send(Self::packet(
                packet_ref.target,
                packet_ref.port,
                packet_ref.payload,
            ))
            .await;

        Ok(packet_ref.payload.len())
    }

    async fn establish_direct(
        proxy: &Proxy,
        session: &Session,
        peer: &TrojanUdpPeer<'_>,
        target: &Address,
        target_port: u16,
    ) -> Result<TrojanEntry, EngineError> {
        use zero_config::ClientTlsConfig;

        let upstream = proxy
            .protocols
            .direct_outbound
            .connect_host(
                peer.endpoint.server,
                peer.endpoint.port,
                proxy.resolver.as_ref(),
            )
            .await?;

        let tls_config = ClientTlsConfig {
            server_name: peer.sni.map(|s| s.to_owned()),
            disable_sni: false,
            ca_cert_path: None,
            insecure: peer.insecure,
            alpn: Vec::new(),
            client_fingerprint: peer.client_fingerprint.map(|s| s.to_owned()),
        };
        let tls_stream = zero_transport::tls::connect_tls_upstream(
            upstream,
            &tls_config,
            proxy.config.source_dir(),
            peer.endpoint.server,
        )
        .await?;

        Self::establish_packet_stream(
            proxy,
            session,
            TcpRelayStream::new(tls_stream),
            peer.password,
            target,
            target_port,
        )
        .await
    }

    async fn establish_over_relay_stream(
        stream: TcpRelayStream,
        tls_server_name: Option<&str>,
        proxy: &Proxy,
        session: &Session,
        peer: &TrojanUdpPeer<'_>,
        target: &Address,
        target_port: u16,
    ) -> Result<TrojanEntry, EngineError> {
        use zero_config::ClientTlsConfig;

        let tls_config = ClientTlsConfig {
            server_name: peer.sni.or(tls_server_name).map(|s| s.to_owned()),
            disable_sni: false,
            ca_cert_path: None,
            insecure: peer.insecure,
            alpn: Vec::new(),
            client_fingerprint: peer.client_fingerprint.map(|s| s.to_owned()),
        };

        let tls_stream = zero_transport::tls::connect_tls_stream(
            stream,
            &tls_config,
            proxy.config.source_dir(),
            peer.endpoint.server,
        )
        .await?;

        Self::establish_packet_stream(
            proxy,
            session,
            tls_stream,
            peer.password,
            target,
            target_port,
        )
        .await
    }

    async fn establish_packet_stream(
        proxy: &Proxy,
        session: &Session,
        stream: TcpRelayStream,
        password: &str,
        _target: &Address,
        _target_port: u16,
    ) -> Result<TrojanEntry, EngineError> {
        let trojan = proxy.protocols.trojan_outbound;
        let mut metered = MeteredStream::new(stream);
        <TrojanOutbound as UdpPacketTunnelProtocol<TrojanUdpPacketTunnelTarget>>::establish_udp_packet_tunnel(
            &trojan,
            &mut metered,
            &TrojanUdpPacketTunnelTarget {
                session,
                password,
            },
        )
        .await?;

        let (read_half, write_half) = tokio::io::split(metered.into_inner());
        let (send_tx, mut send_rx) = mpsc::channel::<TrojanUdpPacket>(32);
        let (recv_tx, _) = broadcast::channel::<TrojanUdpPacket>(32);

        let mut send_stream = WriteOnlySocket(write_half);
        let send_trojan = trojan;
        tokio::spawn(async move {
            while let Some(packet) = send_rx.recv().await {
                if <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::write_udp_packet(
                    &send_trojan,
                    &mut send_stream,
                    &packet,
                )
                .await
                .is_err()
                {
                    break;
                }
            }
        });

        let mut recv_stream = ReadOnlySocket(read_half);
        let recv_tx2 = recv_tx.clone();
        let recv_trojan = trojan;
        tokio::spawn(async move {
            loop {
                match <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::read_udp_packet(
                    &recv_trojan,
                    &mut recv_stream,
                )
                .await
                {
                    Ok(packet) => {
                        if recv_tx2.send(packet).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(TrojanEntry { send_tx, recv_tx })
    }

    fn spawn_bridge(
        chain_tasks: &mut JoinSet<ChainTask>,
        recv_tx: broadcast::Sender<TrojanUdpPacket>,
        session_id: u64,
    ) {
        let mut recv_rx = recv_tx.subscribe();
        chain_tasks.spawn(async move {
            let packet = recv_rx
                .recv()
                .await
                .map_err(|_| EngineError::Io(std::io::Error::other("trojan upstream closed")))?;
            Ok((packet.target, packet.port, packet.payload, Some(session_id)))
        });
    }

    fn packet(target: &Address, port: u16, payload: &[u8]) -> TrojanUdpPacket {
        TrojanUdpPacket {
            target: target.clone(),
            port,
            payload: payload.to_vec(),
        }
    }
}

#[cfg(feature = "trojan")]
struct ReadOnlySocket(ReadHalf<TcpRelayStream>);

#[cfg(feature = "trojan")]
impl AsyncSocket for ReadOnlySocket {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.read(buf).await
    }

    async fn write_all(&mut self, _buf: &[u8]) -> Result<(), Self::Error> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "read-only socket cannot write",
        ))
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(feature = "trojan")]
struct WriteOnlySocket(WriteHalf<TcpRelayStream>);

#[cfg(feature = "trojan")]
impl AsyncSocket for WriteOnlySocket {
    type Error = io::Error;

    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "write-only socket cannot read",
        ))
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.0.write_all(buf).await?;
        self.0.flush().await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.0.shutdown().await
    }
}

#[cfg(not(feature = "trojan"))]
pub(super) struct TrojanChainManager;

#[cfg(not(feature = "trojan"))]
impl TrojanChainManager {
    pub(super) fn new() -> Self {
        Self
    }
    #[allow(unused_variables)]
    pub(super) async fn send(
        &mut self,
        _ctx: UdpFlowContext<'_>,
        _proxy: &Proxy,
        _sess: &Session,
        _peer: TrojanUdpPeer<'_>,
        _packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        Err(FlowFailure {
            stage: "trojan_feature",
            error: EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "Trojan requires feature `trojan`",
            )),
            upstream: None,
        })
    }
}
