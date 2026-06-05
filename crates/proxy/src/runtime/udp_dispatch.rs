//! Generic UDP dispatch: protocol-agnostic routing and outbound dispatch.
//!
//! [`UdpDispatch`] is the single entry point for all UDP packet routing.
//! Each inbound protocol creates a `UdpDispatch` instance, calls [`dispatch()`]
//! for each incoming packet, and polls for responses to deliver to its client.
//!
//! # Supported outbounds
//!
//! All outbound types: direct, block, socks5, vless, shadowsocks, hysteria2,
//! trojan, mieru.
//!
//! # Usage
//!
//! ```ignore
//! let mut dispatch = UdpDispatch::new("inbound-tag").await?;
//!
//! // For each incoming packet:
//! dispatch.dispatch(proxy, target, port, payload, ProtocolType::Vless, auth.as_ref()).await?;
//!
//! // Poll for responses in a select loop:
//! select! {
//!     recv = dispatch.direct_socket().recv_from_addr(&mut buf) => { /* direct response */ }
//!     resp = dispatch.vless_manager().next_response() => { /* VLESS chain */ }
//!     // ...
//! }
//!
//! // Cleanup:
//! for completed in dispatch.finish_all() {
//!     log_completed_udp_flow(completed);
//! }
//! ```

use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Instant;

use tokio::task::JoinSet;
use tokio::time::Instant as TokioInstant;

use zero_core::{Address, Network, ProtocolType, Session, SessionAuth};
use zero_engine::{
    EngineError, ResolvedLeafOutbound, ResolvedOutbound, SessionHandle, SessionOutcome,
};
use zero_platform_tokio::TokioDatagramSocket;
use zero_traits::UdpPacketFraming;

use crate::logging::{log_session_accepted, log_session_failed, log_session_finished};
use crate::runtime::udp_associate::sessions::{
    CompletedUdpFlow, UdpFlowOutbound, UdpFlowSnapshot, UdpSessionFlows,
};
use crate::runtime::udp_helpers::send_direct_udp_packet;
use crate::runtime::vless_udp::{
    establish_vless_udp_upstream_over_stream, VlessUdpOutboundManager, VlessUdpTransport,
};
use crate::runtime::Proxy;

// Chain response types.

/// A response item produced by a chain-outbound recv bridge task.
/// Stored in a unified [`JoinSet`] so all chain outbound responses are
/// polled from a single `select!` branch via [`UdpDispatch::poll_chain_response`].
pub(crate) type ChainTask = Result<(Address, u16, Vec<u8>, Option<u64>), EngineError>;

// SS chain manager.

/// Per-dispatcher manager for Shadowsocks chain outbound.
///
/// Caches upstream UDP sockets per-dispatcher (not globally) and spawns
/// one-shot bridge tasks into the shared [`JoinSet`] so responses are
/// polled uniformly via [`UdpDispatch::poll_chain_response`].
#[cfg(feature = "shadowsocks")]
mod ss_manager {
    use std::collections::{HashMap, VecDeque};
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};

    use tokio::sync::oneshot;
    use tokio::task::JoinSet;
    use zero_core::Address;
    use zero_engine::EngineError;

    use super::{ChainTask, FlowFailure};

    type SsRecvItem = (Address, u16, Vec<u8>);

    struct SsResponseWaiter {
        target: Address,
        port: u16,
        tx: oneshot::Sender<SsRecvItem>,
    }

    struct SsUpstream {
        socket: Arc<tokio::net::UdpSocket>,
        waiters: Mutex<VecDeque<SsResponseWaiter>>,
    }

    pub(super) struct SsChainManager {
        upstreams: HashMap<(String, u16, String, String), Arc<SsUpstream>>,
    }

    impl SsChainManager {
        pub(super) fn new() -> Self {
            Self {
                upstreams: HashMap::new(),
            }
        }

        pub(super) async fn send(
            &mut self,
            chain_tasks: &mut JoinSet<ChainTask>,
            session_id: u64,
            server: &str,
            port: u16,
            password: &str,
            cipher: &str,
            target: &Address,
            target_port: u16,
            payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            use shadowsocks::{
                CipherKind, ShadowsocksOutbound, ShadowsocksUdpDecodeContext,
                ShadowsocksUdpPacketTarget,
            };
            use zero_traits::UdpDatagramFraming;

            let cipher_kind = CipherKind::from_str(cipher).ok_or_else(|| FlowFailure {
                stage: "ss_cipher",
                error: EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown shadowsocks cipher: {cipher}"),
                )),
                upstream: Some((server.to_owned(), port)),
            })?;

            let entry = self.ensure_entry(server, port, password, cipher_kind);

            let packet = <ShadowsocksOutbound as UdpDatagramFraming<
                ShadowsocksUdpPacketTarget,
                ShadowsocksUdpDecodeContext,
            >>::encode_udp_datagram(
                &ShadowsocksOutbound,
                &ShadowsocksUdpPacketTarget {
                    target,
                    port: target_port,
                    payload,
                    cipher: cipher_kind,
                    password: password.as_bytes(),
                },
            )
            .map_err(|e| FlowFailure {
                stage: "ss_encode",
                error: EngineError::Io(std::io::Error::other(e)),
                upstream: Some((server.to_owned(), port)),
            })?;

            let target_addr: SocketAddr =
                format!("{server}:{port}")
                    .parse()
                    .map_err(|_| FlowFailure {
                        stage: "ss_parse_addr",
                        error: EngineError::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            format!("invalid ss upstream: {server}:{port}"),
                        )),
                        upstream: Some((server.to_owned(), port)),
                    })?;

            let (response_tx, response_rx) = oneshot::channel();
            entry
                .waiters
                .lock()
                .expect("ss waiters lock poisoned")
                .push_back(SsResponseWaiter {
                    target: target.clone(),
                    port: target_port,
                    tx: response_tx,
                });
            if let Err(e) = entry.socket.send_to(&packet, target_addr).await {
                remove_waiter(&entry.waiters, target, target_port);
                return Err(FlowFailure {
                    stage: "ss_send",
                    error: EngineError::from(e),
                    upstream: Some((server.to_owned(), port)),
                });
            }

            // Spawn one-shot bridge task.
            chain_tasks.spawn(async move {
                match response_rx.await {
                    Ok((resp_target, resp_port, resp_payload)) => {
                        Ok((resp_target, resp_port, resp_payload, Some(session_id)))
                    }
                    Err(_) => Err(EngineError::Io(std::io::Error::other("ss upstream closed"))),
                }
            });

            Ok(payload.len())
        }

        fn ensure_entry(
            &mut self,
            server: &str,
            port: u16,
            password: &str,
            cipher_kind: shadowsocks::CipherKind,
        ) -> Arc<SsUpstream> {
            let key = (
                server.to_owned(),
                port,
                format!("{cipher_kind:?}"),
                password.to_owned(),
            );
            if let Some(entry) = self.upstreams.get(&key) {
                return entry.clone();
            }

            let socket = Arc::new({
                let socket = std::net::UdpSocket::bind("0.0.0.0:0").expect("ss: bind");
                socket.set_nonblocking(true).expect("ss: nonblocking");
                tokio::net::UdpSocket::from_std(socket).expect("ss: tokio")
            });

            let entry = Arc::new(SsUpstream {
                socket: socket.clone(),
                waiters: Mutex::new(VecDeque::new()),
            });
            self.upstreams.insert(key, entry.clone());

            tokio::spawn(Self::recv_loop(
                socket,
                cipher_kind,
                password.to_owned(),
                entry.clone(),
            ));
            entry
        }

        async fn recv_loop(
            socket: Arc<tokio::net::UdpSocket>,
            cipher: shadowsocks::CipherKind,
            password: String,
            upstream: Arc<SsUpstream>,
        ) {
            use shadowsocks::{
                ShadowsocksOutbound, ShadowsocksUdpDecodeContext, ShadowsocksUdpPacketTarget,
            };
            use zero_traits::UdpDatagramFraming;
            let mut buf = vec![0u8; 4096];
            loop {
                let (n, _) = match socket.recv_from(&mut buf).await {
                    Ok(r) => r,
                    Err(_) => break,
                };
                let packet = &buf[..n];
                let Ok(decoded) = <ShadowsocksOutbound as UdpDatagramFraming<
                    ShadowsocksUdpPacketTarget,
                    ShadowsocksUdpDecodeContext,
                >>::decode_udp_datagram(
                    &ShadowsocksOutbound,
                    &ShadowsocksUdpDecodeContext {
                        cipher,
                        password: password.as_bytes(),
                    },
                    packet,
                ) else {
                    continue;
                };
                let waiter = remove_waiter(&upstream.waiters, &decoded.target, decoded.port);
                if let Some(waiter) = waiter {
                    let _ = waiter
                        .tx
                        .send((decoded.target, decoded.port, decoded.payload));
                }
            }
        }
    }

    fn remove_waiter(
        waiters: &Mutex<VecDeque<SsResponseWaiter>>,
        target: &Address,
        port: u16,
    ) -> Option<SsResponseWaiter> {
        let mut waiters = waiters.lock().expect("ss waiters lock poisoned");
        let index = waiters
            .iter()
            .position(|waiter| waiter.target == *target && waiter.port == port)?;
        waiters.remove(index)
    }
}
#[cfg(feature = "shadowsocks")]
use ss_manager::SsChainManager;

// Trojan chain manager.

#[cfg(feature = "trojan")]
mod trojan_manager {
    use std::collections::HashMap;
    use std::io;

    use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
    use tokio::sync::{broadcast, mpsc};
    use tokio::task::JoinSet;
    use trojan::{TrojanOutbound, TrojanUdpPacket, TrojanUdpPacketTunnelTarget};
    use zero_core::{Address, Session};
    use zero_engine::EngineError;
    use zero_traits::{AsyncSocket, UdpPacketStreamFraming, UdpPacketTunnelProtocol};

    use super::{ChainTask, FlowFailure};
    use crate::runtime::Proxy;
    use crate::transport::{MeteredStream, TcpRelayStream};

    pub(super) struct TrojanChainManager {
        upstreams: HashMap<TrojanKey, TrojanEntry>,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

    struct TrojanEntry {
        send_tx: mpsc::Sender<TrojanUdpPacket>,
        recv_tx: broadcast::Sender<TrojanUdpPacket>,
    }

    impl TrojanChainManager {
        pub(super) fn new() -> Self {
            Self {
                upstreams: HashMap::new(),
            }
        }

        pub(super) async fn send(
            &mut self,
            chain_tasks: &mut JoinSet<ChainTask>,
            session_id: u64,
            proxy: &Proxy,
            session: &Session,
            server: &str,
            port: u16,
            password: &str,
            sni: Option<&str>,
            insecure: bool,
            client_fingerprint: Option<&str>,
            relay_chain: bool,
            target: &Address,
            target_port: u16,
            payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            let sent = payload.len();
            let key = if relay_chain {
                TrojanKey::Relay { session_id }
            } else {
                TrojanKey::Leaf {
                    server: server.to_owned(),
                    port,
                    password: password.to_owned(),
                }
            };

            if let Some(entry) = self.upstreams.get(&key) {
                Self::spawn_bridge(chain_tasks, entry.recv_tx.clone(), session_id);
                let _ = entry
                    .send_tx
                    .send(Self::packet(target, target_port, payload))
                    .await;
                return Ok(sent);
            }

            if relay_chain {
                return Err(FlowFailure {
                    stage: "trojan_relay_upstream",
                    error: EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::NotConnected,
                        "trojan relay upstream is not established",
                    )),
                    upstream: Some((server.to_owned(), port)),
                });
            }

            let entry = Self::establish_direct(
                proxy,
                session,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
                target,
                target_port,
            )
            .await
            .map_err(|e| FlowFailure {
                stage: "trojan_establish",
                error: e,
                upstream: Some((server.to_owned(), port)),
            })?;

            Self::spawn_bridge(chain_tasks, entry.recv_tx.clone(), session_id);
            let send_tx = entry.send_tx.clone();
            self.upstreams.insert(key, entry);

            let _ = send_tx
                .send(Self::packet(target, target_port, payload))
                .await;

            Ok(sent)
        }

        pub(super) async fn send_relay(
            &mut self,
            chain_tasks: &mut JoinSet<ChainTask>,
            session_id: u64,
            stream: TcpRelayStream,
            tls_server_name: Option<&str>,
            proxy: &Proxy,
            session: &Session,
            server: &str,
            port: u16,
            password: &str,
            sni: Option<&str>,
            insecure: bool,
            client_fingerprint: Option<&str>,
            target: &Address,
            target_port: u16,
            payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            let key = TrojanKey::Relay { session_id };
            let entry = Self::establish_over_relay_stream(
                stream,
                tls_server_name,
                proxy,
                session,
                server,
                password,
                sni,
                insecure,
                client_fingerprint,
                target,
                target_port,
            )
            .await
            .map_err(|e| FlowFailure {
                stage: "trojan_relay_establish",
                error: e,
                upstream: Some((server.to_owned(), port)),
            })?;

            Self::spawn_bridge(chain_tasks, entry.recv_tx.clone(), session_id);
            let send_tx = entry.send_tx.clone();
            self.upstreams.insert(key, entry);
            let _ = send_tx
                .send(Self::packet(target, target_port, payload))
                .await;

            Ok(payload.len())
        }

        async fn establish_direct(
            proxy: &Proxy,
            session: &Session,
            server: &str,
            port: u16,
            password: &str,
            sni: Option<&str>,
            insecure: bool,
            client_fingerprint: Option<&str>,
            target: &Address,
            target_port: u16,
        ) -> Result<TrojanEntry, EngineError> {
            use zero_config::ClientTlsConfig;

            let upstream = proxy
                .protocols
                .direct_outbound
                .connect_host(server, port, proxy.resolver.as_ref())
                .await?;

            let tls_config = ClientTlsConfig {
                server_name: sni.map(|s| s.to_owned()),
                disable_sni: false,
                ca_cert_path: None,
                insecure,
                alpn: Vec::new(),
                client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
            };
            let tls_stream = zero_transport::tls::connect_tls_upstream(
                upstream,
                &tls_config,
                proxy.config.source_dir(),
                server,
            )
            .await?;

            Self::establish_packet_stream(
                proxy,
                session,
                TcpRelayStream::new(tls_stream),
                password,
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
            server: &str,
            password: &str,
            sni: Option<&str>,
            insecure: bool,
            client_fingerprint: Option<&str>,
            target: &Address,
            target_port: u16,
        ) -> Result<TrojanEntry, EngineError> {
            use zero_config::ClientTlsConfig;

            let tls_config = ClientTlsConfig {
                server_name: sni.or(tls_server_name).map(|s| s.to_owned()),
                disable_sni: false,
                ca_cert_path: None,
                insecure,
                alpn: Vec::new(),
                client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
            };

            let tls_stream = zero_transport::tls::connect_tls_stream(
                stream,
                &tls_config,
                proxy.config.source_dir(),
                server,
            )
            .await?;

            Self::establish_packet_stream(proxy, session, tls_stream, password, target, target_port)
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
                let packet = recv_rx.recv().await.map_err(|_| {
                    EngineError::Io(std::io::Error::other("trojan upstream closed"))
                })?;
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

    struct ReadOnlySocket(ReadHalf<TcpRelayStream>);

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

    struct WriteOnlySocket(WriteHalf<TcpRelayStream>);

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
}
#[cfg(not(feature = "trojan"))]
mod trojan_manager {
    use super::{ChainTask, FlowFailure};
    use crate::runtime::Proxy;
    use tokio::task::JoinSet;
    use zero_core::{Address, Session};
    pub(super) struct TrojanChainManager;
    impl TrojanChainManager {
        pub(super) fn new() -> Self {
            Self
        }
        #[allow(unused_variables)]
        pub(super) async fn send(
            &mut self,
            _tasks: &mut JoinSet<ChainTask>,
            _sid: u64,
            proxy: &Proxy,
            _sess: &Session,
            _server: &str,
            _port: u16,
            _password: &str,
            _sni: Option<&str>,
            _insecure: bool,
            _fp: Option<&str>,
            _target: &Address,
            _tp: u16,
            _payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            Err(FlowFailure {
                stage: "trojan_feature",
                error: zero_engine::EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Trojan requires feature `trojan`",
                )),
                upstream: None,
            })
        }
    }
}
use trojan_manager::TrojanChainManager;

// Mieru chain manager.

#[cfg(feature = "mieru")]
mod mieru_manager {
    use std::collections::HashMap;
    use std::sync::Arc;

    use mieru::{unwrap_udp_associate, wrap_udp_associate, MieruOutbound};
    use tokio::io::AsyncWriteExt;
    use tokio::sync::{broadcast, mpsc};
    use tokio::task::JoinSet;
    use zero_core::{Address, Session};
    use zero_engine::EngineError;
    use zero_traits::AsyncSocket;

    use super::{ChainTask, FlowFailure};
    use crate::runtime::Proxy;
    use crate::transport::TcpRelayStream;

    type RecvItem = (Address, u16, Vec<u8>);

    pub(super) struct MieruChainManager {
        upstreams: HashMap<(String, u16, String, String), MieruEntry>,
    }

    struct MieruEntry {
        send_tx: mpsc::Sender<Vec<u8>>,
    }

    impl MieruChainManager {
        pub(super) fn new() -> Self {
            Self {
                upstreams: HashMap::new(),
            }
        }

        pub(super) async fn send(
            &mut self,
            chain_tasks: &mut JoinSet<ChainTask>,
            session_id: u64,
            proxy: &Proxy,
            session: &Session,
            server: &str,
            port: u16,
            username: &str,
            password: &str,
            target: &Address,
            target_port: u16,
            payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            let sent = payload.len();
            let key = (
                server.to_owned(),
                port,
                username.to_owned(),
                password.to_owned(),
            );

            if let Some(entry) = self.upstreams.get(&key) {
                let packet =
                    socks5::build_udp_packet(target, target_port, payload).map_err(|error| {
                        FlowFailure {
                            stage: "mieru_udp_packet",
                            error: EngineError::from(error),
                            upstream: Some((server.to_owned(), port)),
                        }
                    })?;
                let wrapped = wrap_udp_associate(&packet);
                let _ = entry.send_tx.send(wrapped).await;
                return Ok(sent);
            }

            // Cache miss: establish new upstream.
            let send_tx = Self::establish(
                proxy,
                chain_tasks,
                session_id,
                session,
                server,
                port,
                username,
                password,
                target,
                target_port,
            )
            .await
            .map_err(|e| FlowFailure {
                stage: "mieru_establish",
                error: e,
                upstream: Some((server.to_owned(), port)),
            })?;

            self.upstreams.insert(
                key,
                MieruEntry {
                    send_tx: send_tx.clone(),
                },
            );

            let packet =
                socks5::build_udp_packet(target, target_port, payload).map_err(|error| {
                    FlowFailure {
                        stage: "mieru_udp_packet",
                        error: EngineError::from(error),
                        upstream: Some((server.to_owned(), port)),
                    }
                })?;
            let wrapped = wrap_udp_associate(&packet);
            let _ = send_tx.send(wrapped).await;
            Ok(sent)
        }

        async fn establish(
            proxy: &Proxy,
            chain_tasks: &mut JoinSet<ChainTask>,
            session_id: u64,
            _session: &Session,
            server: &str,
            port: u16,
            username: &str,
            password: &str,
            _target: &Address,
            _target_port: u16,
        ) -> Result<mpsc::Sender<Vec<u8>>, EngineError> {
            // TCP connect
            let socket = proxy
                .protocols
                .direct_outbound
                .connect_host(server, port, proxy.resolver.as_ref())
                .await?;

            let mut stream = TcpRelayStream::new(socket);

            // Mieru handshake
            let udp_associate_target = Address::Ipv4([0, 0, 0, 0]);
            let outbound =
                MieruOutbound::connect(&mut stream, username, password, &udp_associate_target, 0)
                    .await
                    .map_err(|e| {
                        EngineError::Io(std::io::Error::other(format!("mieru udp handshake: {e}")))
                    })?;

            let (send_tx, mut send_rx) = mpsc::channel::<Vec<u8>>(32);
            let (recv_tx, _) = broadcast::channel::<RecvItem>(32);

            let shared_outbound = Arc::new(tokio::sync::Mutex::new(outbound));
            let shared_stream = Arc::new(tokio::sync::Mutex::new(stream));

            // Send task
            let send_outbound = shared_outbound.clone();
            let send_stream = shared_stream.clone();
            tokio::spawn(async move {
                while let Some(payload) = send_rx.recv().await {
                    let mut ob = send_outbound.lock().await;
                    match ob.encrypt_client_data(&payload) {
                        Ok(encrypted) => {
                            let mut s = send_stream.lock().await;
                            if AsyncWriteExt::write_all(&mut *s, &encrypted).await.is_err() {
                                break;
                            }
                            if AsyncWriteExt::flush(&mut *s).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            // Recv task
            let recv_outbound = shared_outbound.clone();
            let recv_stream = shared_stream.clone();
            let recv_tx2 = recv_tx.clone();
            tokio::spawn(async move {
                let mut raw = Vec::new();
                loop {
                    let mut scratch = [0u8; 4096];
                    let mut s = recv_stream.lock().await;
                    match s.read(&mut scratch).await {
                        Ok(0) => break,
                        Ok(n) => raw.extend_from_slice(&scratch[..n]),
                        Err(_) => break,
                    }
                    loop {
                        let mut ob = recv_outbound.lock().await;
                        match ob.decrypt_server_data_with_consumed(&raw) {
                            Ok((segment, consumed)) => {
                                raw.drain(..consumed);
                                if !segment.payload.is_empty() {
                                    if let Ok(unwrapped) = unwrap_udp_associate(&segment.payload) {
                                        if let Ok(packet) = socks5::parse_udp_packet(&unwrapped) {
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
                            Err(e) if e == zero_core::Error::Protocol("mieru: need more data") => {
                                break
                            }
                            Err(_) => return,
                        }
                    }
                }
            });

            // Spawn one-shot bridge task for the response
            let mut recv_rx = recv_tx.subscribe();
            chain_tasks.spawn(async move {
                let (target, port, payload) = recv_rx
                    .recv()
                    .await
                    .map_err(|_| EngineError::Io(std::io::Error::other("mieru upstream closed")))?;
                Ok((target, port, payload, Some(session_id)))
            });

            Ok(send_tx)
        }
    }
}
#[cfg(not(feature = "mieru"))]
mod mieru_manager {
    use super::{ChainTask, FlowFailure};
    use crate::runtime::Proxy;
    use tokio::task::JoinSet;
    use zero_core::{Address, Session};
    pub(super) struct MieruChainManager;
    impl MieruChainManager {
        pub(super) fn new() -> Self {
            Self
        }
        #[allow(unused_variables)]
        pub(super) async fn send(
            &mut self,
            _tasks: &mut JoinSet<ChainTask>,
            _sid: u64,
            _proxy: &Proxy,
            _sess: &Session,
            _server: &str,
            _port: u16,
            _username: &str,
            _password: &str,
            _target: &Address,
            _tp: u16,
            _payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            Err(FlowFailure {
                stage: "mieru_feature",
                error: zero_engine::EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Mieru requires feature `mieru`",
                )),
                upstream: None,
            })
        }
    }
}
use mieru_manager::MieruChainManager;

// H2 chain manager.

#[cfg(feature = "hysteria2")]
mod h2_manager {
    use std::collections::HashMap;
    use std::sync::Arc;

    use hysteria2::{build_udp_datagram, parse_udp_datagram};
    use tokio::sync::broadcast;
    use tokio::task::JoinSet;
    use zero_core::Address;
    use zero_engine::EngineError;

    use super::{ChainTask, FlowFailure};
    use crate::runtime::Proxy;
    use crate::transport::Hysteria2Connector;

    type RecvItem = (Address, u16, Vec<u8>);

    pub(super) struct H2ChainManager {
        upstreams: HashMap<(String, u16, String), H2Entry>,
    }

    struct H2Entry {
        send_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    }

    impl H2ChainManager {
        pub(super) fn new() -> Self {
            Self {
                upstreams: HashMap::new(),
            }
        }

        pub(super) async fn send(
            &mut self,
            chain_tasks: &mut JoinSet<ChainTask>,
            session_id: u64,
            _proxy: &Proxy,
            server: &str,
            port: u16,
            password: &str,
            client_fingerprint: Option<&str>,
            target: &Address,
            target_port: u16,
            payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            let sent = payload.len();
            let key = (server.to_owned(), port, password.to_owned());

            // Cache hit
            if let Some(entry) = self.upstreams.get(&key) {
                let dg = build_udp_datagram(0, 0, target, target_port, payload)
                    .expect("h2 build datagram");
                let _ = entry.send_tx.send(dg).await;
                return Ok(sent);
            }

            // Cache miss: establish new upstream.
            let send_tx = Self::establish(
                chain_tasks,
                session_id,
                server,
                port,
                password,
                client_fingerprint,
                target,
                target_port,
                payload,
            )
            .await
            .map_err(|e| FlowFailure {
                stage: "h2_establish",
                error: e,
                upstream: Some((server.to_owned(), port)),
            })?;

            self.upstreams.insert(
                key,
                H2Entry {
                    send_tx: send_tx.clone(),
                },
            );

            Ok(sent)
        }

        async fn establish(
            chain_tasks: &mut JoinSet<ChainTask>,
            session_id: u64,
            server: &str,
            port: u16,
            password: &str,
            client_fingerprint: Option<&str>,
            target: &Address,
            target_port: u16,
            initial_payload: &[u8],
        ) -> Result<tokio::sync::mpsc::Sender<Vec<u8>>, EngineError> {
            let connector = Hysteria2Connector::new(server, port, password)
                .with_fingerprint(client_fingerprint);
            let conn = Arc::new(connector.connect_raw().await?);

            let (send_tx, mut send_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(32);
            let (recv_tx, _) = broadcast::channel::<RecvItem>(32);

            let target_owned = target.clone();
            let port_owned = target_port;
            let init_payload = initial_payload.to_vec();

            // Send task: reads outgoing datagrams, sends via QUIC.
            let conn_send = conn.clone();
            tokio::spawn(async move {
                // Send initial payload first.
                if let Ok(dg) = build_udp_datagram(0, 0, &target_owned, port_owned, &init_payload) {
                    if conn_send.send_datagram(dg.into()).is_err() {
                        return;
                    }
                }
                while let Some(datagram) = send_rx.recv().await {
                    if conn_send.send_datagram(datagram.into()).is_err() {
                        break;
                    }
                }
            });

            // Recv task: reads QUIC datagrams, parses target+port, broadcasts.
            let conn_recv = conn.clone();
            let recv_tx2 = recv_tx.clone();
            tokio::spawn(async move {
                loop {
                    match conn_recv.read_datagram().await {
                        Ok(data) => {
                            if let Ok(pkt) = parse_udp_datagram(&data) {
                                if recv_tx2.send((pkt.target, pkt.port, pkt.payload)).is_err() {
                                    break;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            // Spawn one-shot bridge task for the response.
            let mut recv_rx = recv_tx.subscribe();
            chain_tasks.spawn(async move {
                match recv_rx.recv().await {
                    Ok((t, p, payload)) => Ok((t, p, payload, Some(session_id))),
                    Err(_) => Err(EngineError::Io(std::io::Error::other("h2 upstream closed"))),
                }
            });

            Ok(send_tx)
        }
    }
}
#[cfg(not(feature = "hysteria2"))]
mod h2_manager {
    use super::{ChainTask, FlowFailure};
    use crate::runtime::Proxy;
    use tokio::task::JoinSet;
    use zero_core::Address;
    pub(super) struct H2ChainManager;
    impl H2ChainManager {
        pub(super) fn new() -> Self {
            Self
        }
        #[allow(unused_variables)]
        pub(super) async fn send(
            &mut self,
            _tasks: &mut JoinSet<ChainTask>,
            _sid: u64,
            _proxy: &Proxy,
            _server: &str,
            _port: u16,
            _password: &str,
            _fp: Option<&str>,
            _target: &Address,
            _tp: u16,
            _payload: &[u8],
        ) -> Result<usize, FlowFailure> {
            Err(FlowFailure {
                stage: "h2_feature",
                error: zero_engine::EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Hysteria2 requires feature `hysteria2`",
                )),
                upstream: None,
            })
        }
    }
}
use h2_manager::H2ChainManager;

// Types.

/// Result of starting a new UDP flow.
enum FlowStartResult {
    /// A new flow was established and tracked in `UdpSessionFlows`.
    Flow {
        outbound: UdpFlowOutbound,
        tx_bytes: u64,
    },
    /// A VLESS chain flow was established (tracked by the manager, not `UdpSessionFlows`).
    VlessFlow { session_id: u64, tag: String },
    /// The target was blocked.
    Blocked { tag: String },
}

enum UdpCandidate<'a> {
    Leaf(ResolvedLeafOutbound<'a>),
    Relay(Vec<ResolvedLeafOutbound<'a>>),
}

/// Failure details for a flow start attempt.
struct FlowFailure {
    stage: &'static str,
    error: EngineError,
    upstream: Option<(String, u16)>,
}

// UdpDispatch.

/// Protocol-agnostic UDP dispatch state.
///
/// Owns all outbound-specific state (direct socket, upstream associations,
/// VLESS manager) and session flow tracking.  Created per inbound UDP
/// session/association.
pub(crate) struct UdpDispatch {
    inbound_tag: String,
    flows: UdpSessionFlows,
    /// Ephemeral UDP socket for direct outbound (sends to target, receives responses).
    direct_socket: TokioDatagramSocket,
    /// SOCKS5 upstream association (shared across all flows in this session).
    socks5_upstream: Option<crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation>,
    socks5_idle_deadline: Option<TokioInstant>,
    /// VLESS upstream manager (per-target connections).
    vless_manager: VlessUdpOutboundManager,
    /// Session handles for VLESS chain flows. These are not tracked by
    /// [`UdpSessionFlows`] because the VLESS manager owns the per-target
    /// upstream connections. We store handles here so `finish_all()` can
    /// properly complete them.
    vless_handles: HashMap<(Address, u16), (Session, SessionHandle)>,
    /// Unified JoinSet for chain-outbound (SS/H2/Trojan/Mieru/VLESS)
    /// response bridge tasks. Polled by [`poll_chain_response`].
    chain_tasks: JoinSet<ChainTask>,
    /// Per-dispatcher SS chain manager. Caches upstream sockets.
    #[cfg(feature = "shadowsocks")]
    ss_manager: SsChainManager,
    /// Per-dispatcher Trojan chain manager. Caches TLS upstream streams.
    trojan_manager: TrojanChainManager,
    /// Per-dispatcher Mieru chain manager. Caches encrypted upstream streams.
    mieru_manager: MieruChainManager,
    /// Per-dispatcher H2 chain manager. Caches QUIC upstream connections.
    h2_manager: H2ChainManager,
}

impl UdpDispatch {
    /// Create a new dispatcher with an ephemeral direct socket.
    pub(crate) async fn new(inbound_tag: &str) -> Result<Self, EngineError> {
        let direct_socket = TokioDatagramSocket::bind("0.0.0.0:0").await?;
        Ok(Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            socks5_upstream: None,
            socks5_idle_deadline: None,
            vless_manager: VlessUdpOutboundManager::new(),
            vless_handles: HashMap::new(),
            chain_tasks: JoinSet::new(),
            #[cfg(feature = "shadowsocks")]
            ss_manager: SsChainManager::new(),
            trojan_manager: TrojanChainManager::new(),
            mieru_manager: MieruChainManager::new(),
            h2_manager: H2ChainManager::new(),
        })
    }

    /// Create a new dispatcher with a pre-bound direct socket.
    #[allow(dead_code)]
    pub(crate) fn with_socket(inbound_tag: &str, direct_socket: TokioDatagramSocket) -> Self {
        Self {
            inbound_tag: inbound_tag.to_owned(),
            flows: UdpSessionFlows::default(),
            direct_socket,
            socks5_upstream: None,
            socks5_idle_deadline: None,
            vless_manager: VlessUdpOutboundManager::new(),
            vless_handles: HashMap::new(),
            chain_tasks: JoinSet::new(),
            #[cfg(feature = "shadowsocks")]
            ss_manager: SsChainManager::new(),
            trojan_manager: TrojanChainManager::new(),
            mieru_manager: MieruChainManager::new(),
            h2_manager: H2ChainManager::new(),
        }
    }

    // Accessors.

    /// The direct outbound socket. Inbound handlers poll this for direct
    /// responses and use [`direct_response_session_id`] for metering.
    #[allow(dead_code)]
    pub(crate) fn direct_socket(&self) -> &TokioDatagramSocket {
        &self.direct_socket
    }

    /// Borrow direct socket and chain_tasks for `select!` polling.
    pub(crate) fn poll_sockets(&mut self) -> (&TokioDatagramSocket, &mut JoinSet<ChainTask>) {
        (&self.direct_socket, &mut self.chain_tasks)
    }

    /// Borrow all polling sources simultaneously for `select!` loops.
    ///
    /// Returns:
    /// - `&TokioDatagramSocket`: direct outbound response socket
    /// - `Option<&ActiveUpstreamSocks5UdpAssociation>`: SOCKS5 chain upstream
    /// - `Option<TokioInstant>`: SOCKS5 idle deadline
    /// - `&mut JoinSet<ChainTask>`: unified chain response tasks
    pub(crate) fn poll_refs(
        &mut self,
    ) -> (
        &TokioDatagramSocket,
        Option<&crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation>,
        Option<TokioInstant>,
        &mut JoinSet<ChainTask>,
    ) {
        (
            &self.direct_socket,
            self.socks5_upstream.as_ref(),
            self.socks5_idle_deadline,
            &mut self.chain_tasks,
        )
    }

    /// The SOCKS5 upstream association, if established.
    #[allow(dead_code)]
    pub(crate) fn socks5_upstream(
        &self,
    ) -> Option<&crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation> {
        self.socks5_upstream.as_ref()
    }

    /// The SOCKS5 idle deadline.
    #[allow(dead_code)]
    pub(crate) fn socks5_idle_deadline(&self) -> Option<TokioInstant> {
        self.socks5_idle_deadline
    }

    /// Update the SOCKS5 idle deadline (called after each send / recv).
    #[allow(dead_code)]
    pub(crate) fn touch_socks5_idle(&mut self, timeout: std::time::Duration) {
        self.socks5_idle_deadline = Some(TokioInstant::now() + timeout);
    }

    /// Look up the session ID for a direct response sender.
    pub(crate) fn direct_response_session_id(&self, sender: SocketAddr) -> Option<u64> {
        self.flows.direct_response_session_id(sender)
    }

    /// Look up a session ID by target+port only, regardless of outbound type.
    ///
    /// Used for chain-outbound response metering where the outbound tag
    /// may not be known at the call site.
    pub(crate) fn session_id_by_target(&self, target: &Address, port: u16) -> Option<u64> {
        self.flows.session_id_by_target(target, port)
    }

    /// Look up the session ID for an upstream response (requires outbound tag).
    pub(crate) fn upstream_response_session_id(
        &self,
        outbound_tag: &str,
        target: &Address,
        port: u16,
    ) -> Option<u64> {
        self.flows
            .upstream_response_session_id(outbound_tag, target, port)
    }

    /// Take and close the SOCKS5 upstream association (for idle timeout / error).
    #[allow(dead_code)]
    pub(crate) fn take_socks5_upstream(
        &mut self,
    ) -> Option<crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation> {
        self.socks5_idle_deadline = None;
        self.socks5_upstream.take()
    }

    /// Close the SOCKS5 upstream association on idle timeout.
    #[allow(dead_code)]
    pub(crate) fn close_socks5_idle(&mut self) {
        use crate::logging::log_udp_upstream_association_idle_timeout;
        use crate::outbound::socks5::UpstreamAssociationCloseReason;

        if let Some(assoc) = self.socks5_upstream.take() {
            let outbound_tag = assoc.outbound_tag().to_owned();
            let (server, port) = assoc.upstream_endpoint();
            let server = server.to_owned();
            assoc.close(UpstreamAssociationCloseReason::IdleTimeout);
            log_udp_upstream_association_idle_timeout(
                &self.inbound_tag,
                &outbound_tag,
                &server,
                port,
                std::time::Duration::default(), // caller should log with actual timeout
            );
            self.socks5_idle_deadline = None;
        }
    }

    /// Finish all tracked flows and close upstreams.
    ///
    /// Closes the SOCKS5 upstream association, finishes VLESS chain flow
    /// session handles, and drains all regular flows from `UdpSessionFlows`.
    pub(crate) fn finish_all(mut self) -> Vec<CompletedUdpFlow> {
        if let Some(assoc) = self.socks5_upstream {
            use crate::outbound::socks5::UpstreamAssociationCloseReason;
            assoc.close(UpstreamAssociationCloseReason::Closed);
        }

        // Finish VLESS chain flow session handles.
        for (_key, (session, mut handle)) in self.vless_handles.drain() {
            if let Some(record) = handle.finish(SessionOutcome::ChainedRelayed) {
                log_session_finished(&record, None);
                let _ = session; // session was moved into the record
            }
        }

        self.flows.finish_all()
    }

    // Dispatch.

    /// Dispatch a UDP packet: route, select outbound, send.
    ///
    /// If a flow already exists for `(target, port)` (including VLESS chain
    /// connections cached in the manager), forwards the payload.  Otherwise
    /// creates a new session, routes through the engine, and dispatches to
    /// the resolved outbound.
    ///
    /// Supports all outbound types: direct, block, socks5, vless,
    /// shadowsocks, hysteria2, trojan, mieru.
    /// Returns the session_id of the (new or cached) flow for metering.
    pub(crate) async fn dispatch(
        &mut self,
        proxy: &Proxy,
        target: Address,
        port: u16,
        payload: &[u8],
        protocol: ProtocolType,
        auth: Option<&SessionAuth>,
    ) -> Result<u64, EngineError> {
        // VLESS manager shortcut (cached upstream).
        if let Some(handle) = self.vless_manager.get(&target, port) {
            proxy.record_session_inbound_rx(handle.session_id, payload.len() as u64);
            let packet = <vless::VlessOutbound as UdpPacketFraming<
                vless::VlessUdpPacketTarget,
            >>::encode_udp_packet(
                &proxy.protocols.vless_outbound,
                &vless::VlessUdpPacketTarget {
                    address: &target,
                    port,
                    payload,
                },
            )?;
            let packet_len = packet.len() as u64;
            let _ = handle.send_tx.send(packet).await;
            proxy.record_session_outbound_tx(handle.session_id, packet_len);
            // Spawn bridge task for the expected response.
            self.vless_manager
                .spawn_bridge(&mut self.chain_tasks, target, port, handle.session_id);
            return Ok(handle.session_id);
        }

        // Existing flow (direct / socks5 / ss / h2 / trojan / mieru).
        if let Some(flow) = self.flows.snapshot(&target, port) {
            self.forward_existing(proxy, &flow, payload).await?;
            return Ok(flow.session.id);
        }

        // New flow.
        let mut session = Session::new(0, target, port, Network::Udp, protocol);
        if let Some(a) = auth {
            session.auth = Some(a.clone());
        }
        proxy.prepare_session(&mut session, &self.inbound_tag, None);
        let mut session_handle = proxy.track_session(session.id);
        let started_at = Instant::now();
        proxy.record_session_inbound_rx(session.id, payload.len() as u64);

        proxy.resolve_fake_ip_target(&mut session).await;
        let action = proxy.route_decision(&session);
        let resolved = match proxy.resolve_outbound(&action) {
            Ok((resolved, _plan)) => resolved,
            Err(error) => {
                let record = session_handle.finish(SessionOutcome::Failed);
                log_session_failed(
                    &session,
                    record.as_ref(),
                    "resolve_outbound",
                    started_at.elapsed(),
                    &error,
                    None,
                );
                return Err(error);
            }
        };
        log_session_accepted(&session, &action, proxy.config.mode.kind());

        let candidates = match resolved {
            ResolvedOutbound::Single(c) => vec![UdpCandidate::Leaf(c)],
            ResolvedOutbound::Fallback { candidates } => {
                candidates.into_iter().map(UdpCandidate::Leaf).collect()
            }
            ResolvedOutbound::Relay { chain } => vec![UdpCandidate::Relay(chain)],
        };
        let is_fallback = candidates.len() > 1;
        let mut last_failure = None;

        for candidate in candidates {
            match self.start_flow(proxy, candidate, &session, payload).await {
                Ok(FlowStartResult::Flow { outbound, tx_bytes }) => {
                    let session_id = session.id;
                    session.outbound_tag = Some(outbound.tag().to_owned());
                    proxy.set_session_outbound(&session);
                    self.flows.insert(session, session_handle, outbound);
                    proxy.record_session_outbound_tx(session_id, tx_bytes);
                    return Ok(session_id);
                }
                Ok(FlowStartResult::VlessFlow { session_id, tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    self.vless_handles.insert(
                        (session.target.clone(), session.port),
                        (session, session_handle),
                    );
                    proxy.record_session_outbound_tx(session_id, payload.len() as u64);
                    return Ok(session_id);
                }
                Ok(FlowStartResult::Blocked { tag }) => {
                    session.outbound_tag = Some(tag);
                    proxy.set_session_outbound(&session);
                    if let Some(record) = session_handle.finish(SessionOutcome::Blocked) {
                        log_session_finished(&record, None);
                    }
                    return Ok(session.id);
                }
                Err(failure) => {
                    last_failure = Some(failure);
                }
            }
        }

        let record = session_handle.finish(SessionOutcome::Failed);
        if let Some(failure) = last_failure {
            let stage = if is_fallback {
                "fallback_exhausted"
            } else {
                failure.stage
            };
            log_session_failed(
                &session,
                record.as_ref(),
                stage,
                started_at.elapsed(),
                &failure.error,
                failure
                    .upstream
                    .as_ref()
                    .map(|(server, port)| (server.as_str(), *port)),
            );
            return Err(failure.error);
        }

        let error = EngineError::Io(std::io::Error::other("all fallback outbounds failed"));
        log_session_failed(
            &session,
            record.as_ref(),
            "fallback_exhausted",
            started_at.elapsed(),
            &error,
            None,
        );
        Err(error)
    }

    /// Forward a packet to an existing flow.
    async fn forward_existing(
        &mut self,
        proxy: &Proxy,
        flow: &UdpFlowSnapshot,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let started_at = Instant::now();
        proxy.record_session_inbound_rx(flow.session.id, payload.len() as u64);

        match &flow.outbound {
            UdpFlowOutbound::Direct { target_addr, .. } => {
                match send_direct_udp_packet(&self.direct_socket, *target_addr, payload).await {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(error) => {
                        self.fail_flow(&flow, started_at, "udp_direct_send", &error);
                        return Err(error);
                    }
                }
            }
            UdpFlowOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => match self
                .send_socks5(
                    proxy,
                    tag,
                    server,
                    *port,
                    username.as_deref(),
                    password.as_deref(),
                    &flow.session,
                    payload,
                )
                .await
            {
                Ok(sent) => {
                    proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                }
                Err(error) => {
                    self.fail_flow(&flow, started_at, "udp_upstream_send", &error);
                    return Err(error);
                }
            },
            #[cfg(feature = "shadowsocks")]
            UdpFlowOutbound::Shadowsocks {
                tag: _,
                server,
                port,
                password,
                cipher,
            } => {
                match self
                    .ss_manager
                    .send(
                        &mut self.chain_tasks,
                        flow.session.id,
                        server.as_str(),
                        *port,
                        password.as_str(),
                        cipher.as_str(),
                        &flow.session.target,
                        flow.session.port,
                        payload,
                    )
                    .await
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        self.fail_flow_with_msg(
                            &flow,
                            started_at,
                            failure.stage,
                            &failure.error.to_string(),
                        );
                        return Err(failure.error);
                    }
                }
            }
            #[cfg(not(feature = "shadowsocks"))]
            UdpFlowOutbound::Shadowsocks { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Shadowsocks UDP outbound requires feature `shadowsocks`",
                )));
            }
            #[cfg(feature = "hysteria2")]
            UdpFlowOutbound::Hysteria2 {
                tag: _,
                server,
                port,
                password,
                client_fingerprint,
            } => {
                match self
                    .h2_manager
                    .send(
                        &mut self.chain_tasks,
                        flow.session.id,
                        proxy,
                        server.as_str(),
                        *port,
                        password.as_str(),
                        client_fingerprint.as_deref(),
                        &flow.session.target,
                        flow.session.port,
                        payload,
                    )
                    .await
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        self.fail_flow_with_msg(
                            &flow,
                            started_at,
                            failure.stage,
                            &failure.error.to_string(),
                        );
                        return Err(failure.error);
                    }
                }
            }
            #[cfg(not(feature = "hysteria2"))]
            UdpFlowOutbound::Hysteria2 { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Hysteria2 UDP outbound requires feature `hysteria2`",
                )));
            }
            #[cfg(feature = "trojan")]
            UdpFlowOutbound::Trojan {
                tag: _,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
                relay_chain,
            } => {
                match self
                    .trojan_manager
                    .send(
                        &mut self.chain_tasks,
                        flow.session.id,
                        proxy,
                        &flow.session,
                        server.as_str(),
                        *port,
                        password.as_str(),
                        sni.as_deref(),
                        *insecure,
                        client_fingerprint.as_deref(),
                        *relay_chain,
                        &flow.session.target,
                        flow.session.port,
                        payload,
                    )
                    .await
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        self.fail_flow_with_msg(
                            &flow,
                            started_at,
                            failure.stage,
                            &failure.error.to_string(),
                        );
                        return Err(failure.error);
                    }
                }
            }
            #[cfg(not(feature = "trojan"))]
            UdpFlowOutbound::Trojan { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Trojan UDP outbound requires feature `trojan`",
                )));
            }
            #[cfg(feature = "mieru")]
            UdpFlowOutbound::Mieru {
                tag: _,
                server,
                port,
                username,
                password,
            } => {
                match self
                    .mieru_manager
                    .send(
                        &mut self.chain_tasks,
                        flow.session.id,
                        proxy,
                        &flow.session,
                        server.as_str(),
                        *port,
                        username.as_str(),
                        password.as_str(),
                        &flow.session.target,
                        flow.session.port,
                        payload,
                    )
                    .await
                {
                    Ok(sent) => {
                        proxy.record_session_outbound_tx(flow.session.id, sent as u64);
                    }
                    Err(failure) => {
                        self.fail_flow_with_msg(
                            &flow,
                            started_at,
                            failure.stage,
                            &failure.error.to_string(),
                        );
                        return Err(failure.error);
                    }
                }
            }
            #[cfg(not(feature = "mieru"))]
            UdpFlowOutbound::Mieru { .. } => {
                return Err(EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "Mieru UDP outbound requires feature `mieru`",
                )));
            }
        }

        Ok(())
    }

    /// Start a new UDP flow by dispatching to the resolved outbound.
    async fn start_flow(
        &mut self,
        proxy: &Proxy,
        candidate: UdpCandidate<'_>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let candidate = match candidate {
            UdpCandidate::Leaf(candidate) => candidate,
            UdpCandidate::Relay(chain) => {
                return self.start_relay_flow(proxy, chain, session, payload).await;
            }
        };

        match candidate {
            ResolvedLeafOutbound::Direct { tag } => {
                let target_addr = proxy
                    .protocols
                    .direct_outbound
                    .resolve_target_addr(session, proxy.resolver.as_ref())
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "resolve_udp_target",
                        error: error.into(),
                        upstream: None,
                    })?;

                let sent = self
                    .direct_socket
                    .send_to_addr(payload, target_addr)
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_direct_send",
                        error: error.into(),
                        upstream: None,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Direct {
                        tag: tag.unwrap_or("direct").to_owned(),
                        target_addr,
                    },
                    tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Block { tag } => Ok(FlowStartResult::Blocked {
                tag: tag.unwrap_or("block").to_owned(),
            }),
            ResolvedLeafOutbound::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let sent = self
                    .send_socks5(
                        proxy, tag, server, port, username, password, session, payload,
                    )
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_upstream_send",
                        error,
                        upstream: Some((server.to_owned(), port)),
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Socks5 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.map(ToOwned::to_owned),
                        password: password.map(ToOwned::to_owned),
                    },
                    tx_bytes: sent as u64,
                })
            }
            ResolvedLeafOutbound::Vless {
                tag,
                server,
                port,
                id,
                tls,
                reality,
                ws,
                grpc,
                h2,
                http_upgrade,
                split_http,
                quic,
                ..
            } => {
                let transport = VlessUdpTransport {
                    tls,
                    reality,
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    quic,
                };
                let session_id = session.id;
                let tag_owned = tag.to_owned();
                self.vless_manager
                    .get_or_create_upstream(
                        &mut self.chain_tasks,
                        proxy,
                        session,
                        session.target.clone(),
                        session.port,
                        server.to_owned(),
                        port,
                        id.to_owned(),
                        payload.to_vec(),
                        Some(&transport),
                    )
                    .await
                    .map_err(|error| FlowFailure {
                        stage: "udp_vless_upstream",
                        error,
                        upstream: Some((server.to_owned(), port)),
                    })?;

                Ok(FlowStartResult::VlessFlow {
                    session_id,
                    tag: tag_owned,
                })
            }
            #[cfg(feature = "hysteria2")]
            ResolvedLeafOutbound::Hysteria2 {
                tag,
                server,
                port,
                password,
                client_fingerprint,
                ..
            } => {
                let sent = self
                    .h2_manager
                    .send(
                        &mut self.chain_tasks,
                        session.id,
                        proxy,
                        server,
                        port,
                        password,
                        client_fingerprint,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await
                    .map_err(|f: FlowFailure| FlowFailure {
                        stage: f.stage,
                        error: f.error,
                        upstream: f.upstream,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Hysteria2 {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "hysteria2"))]
            ResolvedLeafOutbound::Hysteria2 { .. } => Err(FlowFailure {
                stage: "udp_hysteria2_outbound",
                error: zero_core::Error::Unsupported(
                    "Hysteria2 UDP outbound requires Cargo feature `hysteria2`",
                )
                .into(),
                upstream: None,
            }),
            #[allow(unused_variables)]
            ResolvedLeafOutbound::Shadowsocks {
                tag,
                server,
                port,
                password,
                cipher,
                ..
            } => {
                #[cfg(feature = "shadowsocks")]
                {
                    let sent = self
                        .ss_manager
                        .send(
                            &mut self.chain_tasks,
                            session.id,
                            server,
                            port,
                            password,
                            cipher,
                            &session.target,
                            session.port,
                            payload,
                        )
                        .await
                        .map_err(|f: FlowFailure| FlowFailure {
                            stage: f.stage,
                            error: f.error,
                            upstream: f.upstream,
                        })?;

                    Ok(FlowStartResult::Flow {
                        outbound: UdpFlowOutbound::Shadowsocks {
                            tag: tag.to_owned(),
                            server: server.to_owned(),
                            port,
                            password: password.to_owned(),
                            cipher: cipher.to_owned(),
                        },
                        tx_bytes: sent as u64,
                    })
                }
                #[cfg(not(feature = "shadowsocks"))]
                {
                    Err(FlowFailure {
                        stage: "udp_shadowsocks_outbound",
                        error: zero_core::Error::Unsupported(
                            "Shadowsocks UDP outbound requires Cargo feature `shadowsocks`",
                        )
                        .into(),
                        upstream: None,
                    })
                }
            }
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Trojan {
                tag,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
            } => {
                let sent = self
                    .trojan_manager
                    .send(
                        &mut self.chain_tasks,
                        session.id,
                        proxy,
                        session,
                        server,
                        port,
                        password,
                        sni,
                        insecure,
                        client_fingerprint,
                        false,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await
                    .map_err(|f: FlowFailure| FlowFailure {
                        stage: f.stage,
                        error: f.error,
                        upstream: f.upstream,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Trojan {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        sni: sni.map(|s| s.to_owned()),
                        insecure,
                        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
                        relay_chain: false,
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "trojan"))]
            ResolvedLeafOutbound::Trojan { .. } => Err(FlowFailure {
                stage: "udp_trojan_outbound",
                error: zero_core::Error::Unsupported(
                    "Trojan UDP outbound requires Cargo feature `trojan`",
                )
                .into(),
                upstream: None,
            }),
            #[cfg(feature = "mieru")]
            ResolvedLeafOutbound::Mieru {
                tag,
                server,
                port,
                username,
                password,
            } => {
                let sent = self
                    .mieru_manager
                    .send(
                        &mut self.chain_tasks,
                        session.id,
                        proxy,
                        session,
                        server,
                        port,
                        username,
                        password,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await
                    .map_err(|f: FlowFailure| FlowFailure {
                        stage: f.stage,
                        error: f.error,
                        upstream: f.upstream,
                    })?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Mieru {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        username: username.to_owned(),
                        password: password.to_owned(),
                    },
                    tx_bytes: sent as u64,
                })
            }
            #[cfg(not(feature = "mieru"))]
            ResolvedLeafOutbound::Mieru { .. } => Err(FlowFailure {
                stage: "udp_mieru_outbound",
                error: zero_core::Error::Unsupported(
                    "Mieru UDP outbound requires Cargo feature `mieru`",
                )
                .into(),
                upstream: None,
            }),
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Vmess { .. } => Err(FlowFailure {
                stage: "vmess",
                error: zero_core::Error::Unsupported("vmess UDP not supported").into(),
                upstream: None,
            }),
            #[cfg(not(feature = "trojan"))]
            ResolvedLeafOutbound::Vmess { .. } => Err(FlowFailure {
                stage: "vmess",
                error: zero_core::Error::Unsupported("vmess UDP not supported").into(),
                upstream: None,
            }),
        }
    }

    async fn start_relay_flow(
        &mut self,
        proxy: &Proxy,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        session: &Session,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let (stream, final_hop) = proxy
            .establish_relay_prefix(chain)
            .await
            .map_err(|failure| FlowFailure {
                stage: failure.stage,
                error: failure.error,
                upstream: failure.upstream_endpoint,
            })?;

        match final_hop {
            ResolvedLeafOutbound::Vless {
                tag,
                server,
                port,
                id,
                tls,
                reality,
                ws,
                grpc,
                h2,
                http_upgrade,
                split_http,
                quic,
                ..
            } => {
                if quic.is_some() {
                    return Err(FlowFailure {
                        stage: "udp_relay_final_transport",
                        error: zero_core::Error::Unsupported(
                            "VLESS QUIC final hop over TCP relay chain is not supported",
                        )
                        .into(),
                        upstream: None,
                    });
                }

                let session_id = session.id;
                let tag_owned = tag.to_owned();
                let key = (session.target.clone(), session.port);
                let stream = crate::transport::build_vless_outbound_transport_over_stream(
                    stream,
                    tls,
                    reality,
                    ws,
                    grpc,
                    h2,
                    http_upgrade,
                    split_http,
                    proxy.config.source_dir(),
                    server,
                    port,
                )
                .await
                .map_err(|error| FlowFailure {
                    stage: "udp_relay_final_transport",
                    error,
                    upstream: Some((server.to_owned(), port)),
                })?;
                let (upstream, recv_tx) =
                    establish_vless_udp_upstream_over_stream(proxy, session, id, payload, stream)
                        .await
                        .map_err(|error| FlowFailure {
                            stage: "udp_vless_relay_chain",
                            error,
                            upstream: None,
                        })?;
                self.vless_manager.insert_upstream(key, upstream, recv_tx);
                self.vless_manager.spawn_bridge(
                    &mut self.chain_tasks,
                    session.target.clone(),
                    session.port,
                    session_id,
                );

                Ok(FlowStartResult::VlessFlow {
                    session_id,
                    tag: tag_owned,
                })
            }
            #[cfg(feature = "trojan")]
            ResolvedLeafOutbound::Trojan {
                tag,
                server,
                port,
                password,
                sni,
                insecure,
                client_fingerprint,
            } => {
                let sent = self
                    .trojan_manager
                    .send_relay(
                        &mut self.chain_tasks,
                        session.id,
                        stream,
                        None,
                        proxy,
                        session,
                        server,
                        port,
                        password,
                        sni,
                        insecure,
                        client_fingerprint,
                        &session.target,
                        session.port,
                        payload,
                    )
                    .await?;

                Ok(FlowStartResult::Flow {
                    outbound: UdpFlowOutbound::Trojan {
                        tag: tag.to_owned(),
                        server: server.to_owned(),
                        port,
                        password: password.to_owned(),
                        sni: sni.map(|s| s.to_owned()),
                        insecure,
                        client_fingerprint: client_fingerprint.map(|s| s.to_owned()),
                        relay_chain: true,
                    },
                    tx_bytes: sent as u64,
                })
            }
            _ => Err(FlowFailure {
                stage: "udp_relay_final_hop",
                error: zero_core::Error::Unsupported(
                    "UDP relay chain final hop does not support stream packet UDP",
                )
                .into(),
                upstream: None,
            }),
        }
    }

    // SOCKS5 helper.

    /// Send via SOCKS5 upstream association, establishing one if needed.
    async fn send_socks5(
        &mut self,
        proxy: &Proxy,
        tag: &str,
        server: &str,
        port: u16,
        username: Option<&str>,
        password: Option<&str>,
        session: &Session,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        use crate::logging::log_udp_upstream_association_dropped;
        use crate::outbound::socks5::{
            send_socks5_udp_packet, Socks5UdpAssociation, UpstreamAssociationCloseReason,
        };

        let association = Socks5UdpAssociation {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            auth: username
                .zip(password)
                .map(|(u, p)| (u.to_owned(), p.to_owned())),
        };

        match send_socks5_udp_packet(
            proxy,
            &self.inbound_tag,
            &association,
            session,
            payload,
            &mut self.socks5_upstream,
            &mut self.socks5_idle_deadline,
        )
        .await
        {
            Ok(sent) => {
                // packet_sent already recorded in send_socks5_udp_packet
                Ok(sent)
            }
            Err(error) => {
                if let Some(assoc) = self.socks5_upstream.take() {
                    let outbound_tag = assoc.outbound_tag().to_owned();
                    let (svr, p) = assoc.upstream_endpoint();
                    let svr = svr.to_owned();
                    assoc.close(UpstreamAssociationCloseReason::Dropped);
                    log_udp_upstream_association_dropped(
                        &self.inbound_tag,
                        &outbound_tag,
                        &svr,
                        p,
                        &error,
                    );
                }
                self.socks5_idle_deadline = None;
                proxy.record_udp_upstream_send_failure();
                Err(error)
            }
        }
    }

    // Failure helpers.

    fn fail_flow(
        &mut self,
        flow: &UdpFlowSnapshot,
        started_at: Instant,
        stage: &'static str,
        error: &EngineError,
    ) {
        if let Some(completed) = self.flows.finish(
            &flow.session.target,
            flow.session.port,
            SessionOutcome::Failed,
        ) {
            log_session_failed(
                &flow.session,
                Some(&completed.record),
                stage,
                started_at.elapsed(),
                error,
                None,
            );
        } else {
            log_session_failed(
                &flow.session,
                None,
                stage,
                started_at.elapsed(),
                error,
                None,
            );
        }
    }

    fn fail_flow_with_msg(
        &mut self,
        flow: &UdpFlowSnapshot,
        started_at: Instant,
        stage: &'static str,
        msg: &str,
    ) {
        let error = EngineError::Io(std::io::Error::other(msg.to_string()));
        self.fail_flow(flow, started_at, stage, &error);
    }
}
