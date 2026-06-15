use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use tokio::net::UdpSocket;
use tokio::sync::oneshot;
use tracing::{debug, warn};
use zero_core::Address;
use zero_engine::EngineError;

use super::{DatagramCodec, FlowFailure, UdpFlowContext, UdpPacketPath, UdpPacketRef};
#[cfg(feature = "socks5")]
use crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation;
use crate::runtime::Proxy;

type RecvItem = (Address, u16, Vec<u8>);

#[cfg(feature = "socks5")]
pub(super) struct Socks5PacketPath {
    association: Arc<ActiveUpstreamSocks5UdpAssociation>,
}

#[cfg(feature = "socks5")]
impl UdpPacketPath<Address> for Socks5PacketPath {
    type Error = EngineError;

    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        self.association.send_packet(target, port, payload).await?;
        Ok(())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        let read = self.association.recv_packet(buf).await?;
        let packet = socks5::parse_udp_packet(&buf[..read])
            .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
        let len = packet.payload.len();
        buf[..len].copy_from_slice(&packet.payload);
        Ok(len)
    }
}

pub(super) struct ShadowsocksPacketPath {
    socket: UdpSocket,
    endpoint: SocketAddr,
    codec: shadowsocks::ShadowsocksDatagramCodec,
}

impl ShadowsocksPacketPath {
    async fn establish(
        proxy: &Proxy,
        server: &str,
        port: u16,
        password: &str,
        cipher: shadowsocks::CipherKind,
    ) -> Result<Self, EngineError> {
        let endpoint = proxy
            .protocols
            .direct_outbound
            .resolve_address(
                &Address::Domain(server.to_owned()),
                port,
                proxy.resolver.as_ref(),
                "failed to resolve shadowsocks packet path carrier",
            )
            .await?;
        let bind_addr = match endpoint {
            SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)
            .await
            .map_err(EngineError::from)?;
        Ok(Self {
            socket,
            endpoint,
            codec: shadowsocks::ShadowsocksDatagramCodec {
                cipher,
                password: password.as_bytes().to_vec(),
            },
        })
    }
}

impl UdpPacketPath<Address> for ShadowsocksPacketPath {
    type Error = EngineError;

    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let packet = self
            .codec
            .encode(target, port, payload)
            .map_err(EngineError::from)?;
        self.socket
            .send_to(&packet, self.endpoint)
            .await
            .map_err(EngineError::from)?;
        Ok(())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        let (read, _) = self
            .socket
            .recv_from(buf)
            .await
            .map_err(EngineError::from)?;
        let decoded = self.codec.decode(&buf[..read]).ok_or_else(|| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "failed to decode shadowsocks packet path response",
            ))
        })?;
        let len = decoded.2.len();
        buf[..len].copy_from_slice(&decoded.2);
        Ok(len)
    }
}

/// QUIC-backed packet path carrier for Hysteria2.
///
/// Carries inner-datagram traffic (e.g. Shadowsocks) over a Hysteria2 QUIC
/// connection: `send_to` encodes `(target, port, payload)` with the Hysteria2
/// datagram framing and sends it as a QUIC datagram; `recv_from` reads the
/// next QUIC datagram and returns the decoded payload. Mirrors
/// [`ShadowsocksPacketPath`] but over `quinn::Connection` instead of a raw
/// `UdpSocket`. The underlying QUIC connection is established via
/// [`Hysteria2Connector`](crate::transport::Hysteria2Connector), reusing the
/// same auth + optional TLS fingerprint path as the single-hop outbound.
#[cfg(feature = "hysteria2")]
pub(super) struct Hysteria2PacketPath {
    conn: std::sync::Arc<quinn::Connection>,
}

#[cfg(feature = "hysteria2")]
impl Hysteria2PacketPath {
    async fn establish(
        server: &str,
        port: u16,
        password: &str,
        client_fingerprint: Option<&str>,
    ) -> Result<Self, EngineError> {
        let connector = crate::transport::Hysteria2Connector::new(server, port, password)
            .with_fingerprint(client_fingerprint);
        let conn = std::sync::Arc::new(connector.connect_raw().await?);
        Ok(Self { conn })
    }

    fn encode(target: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, EngineError> {
        use hysteria2::{Hysteria2Outbound, Hysteria2UdpPacketTarget};
        use zero_traits::UdpDatagramFraming;
        <Hysteria2Outbound as UdpDatagramFraming<Hysteria2UdpPacketTarget<'_>, ()>>::encode_udp_datagram(
            &Hysteria2Outbound,
            &Hysteria2UdpPacketTarget {
                session_id: 0,
                packet_id: 0,
                target,
                port,
                payload,
            },
        )
        .map_err(EngineError::from)
    }

    fn decode(data: &[u8]) -> Result<Vec<u8>, EngineError> {
        use hysteria2::{Hysteria2Outbound, Hysteria2UdpPacketTarget};
        use zero_traits::UdpDatagramFraming;
        let pkt =
            <Hysteria2Outbound as UdpDatagramFraming<Hysteria2UdpPacketTarget<'_>, ()>>::decode_udp_datagram(
                &Hysteria2Outbound,
                &(),
                data,
            )
            .map_err(EngineError::from)?;
        Ok(pkt.payload)
    }
}

#[cfg(feature = "hysteria2")]
impl UdpPacketPath<Address> for Hysteria2PacketPath {
    type Error = EngineError;

    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let datagram = Self::encode(target, port, payload)?;
        self.conn.send_datagram(datagram.into()).map_err(|e| {
            EngineError::Io(std::io::Error::other(format!(
                "hysteria2 carrier send: {e}"
            )))
        })?;
        Ok(())
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        let data = self.conn.read_datagram().await.map_err(|e| {
            EngineError::Io(std::io::Error::other(format!(
                "hysteria2 carrier recv: {e}"
            )))
        })?;
        let payload = Self::decode(&data)?;
        let len = payload.len();
        if len > buf.len() {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "hysteria2 carrier datagram ({len}B) exceeds recv buffer ({}B)",
                    buf.len()
                ),
            )));
        }
        buf[..len].copy_from_slice(&payload);
        Ok(len)
    }
}

enum PacketPath {
    #[cfg(feature = "socks5")]
    Socks5(Socks5PacketPath),
    Shadowsocks(ShadowsocksPacketPath),
    #[cfg(feature = "hysteria2")]
    Hysteria2(Hysteria2PacketPath),
}

impl UdpPacketPath<Address> for PacketPath {
    type Error = EngineError;

    async fn send_to(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        match self {
            #[cfg(feature = "socks5")]
            Self::Socks5(path) => path.send_to(target, port, payload).await,
            Self::Shadowsocks(path) => path.send_to(target, port, payload).await,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2(path) => path.send_to(target, port, payload).await,
        }
    }

    async fn recv_from(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        match self {
            #[cfg(feature = "socks5")]
            Self::Socks5(path) => path.recv_from(buf).await,
            Self::Shadowsocks(path) => path.recv_from(buf).await,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2(path) => path.recv_from(buf).await,
        }
    }
}

struct Waiter {
    target: Address,
    port: u16,
    tx: oneshot::Sender<RecvItem>,
}

struct Entry {
    path: Arc<PacketPath>,
    waiters: Arc<Mutex<VecDeque<Waiter>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CarrierKey {
    #[cfg(feature = "socks5")]
    Socks5 {
        tag: String,
        server: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    Shadowsocks {
        tag: String,
        server: String,
        port: u16,
        password: String,
        cipher: String,
    },
    #[cfg(feature = "hysteria2")]
    Hysteria2 {
        tag: String,
        server: String,
        port: u16,
        password: String,
        client_fingerprint: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PathKey {
    carrier: CarrierKey,
    datagram_server: String,
    datagram_port: u16,
    datagram_password: String,
    datagram_cipher: String,
}

pub(super) struct PacketPathManager {
    upstreams: HashMap<PathKey, Entry>,
}

pub(super) enum PacketPathCarrierParams<'a> {
    #[cfg(feature = "socks5")]
    Socks5 {
        tag: &'a str,
        server: &'a str,
        port: u16,
        username: Option<&'a str>,
        password: Option<&'a str>,
    },
    Shadowsocks {
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        cipher: &'a str,
    },
    #[cfg(feature = "hysteria2")]
    Hysteria2 {
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        client_fingerprint: Option<&'a str>,
    },
}

/// Resolved parameters for a datagram-over-packet-path relay chain.
///
/// Produced by [`super::resolve_udp_packet_path_chain`] from a resolved
/// outbound chain. Contains both the carrier packet-path parameters and the
/// inner datagram protocol parameters.
pub(super) struct PacketPathChainParams<'a> {
    pub(super) datagram_tag: &'a str,
    pub(super) carrier: PacketPathCarrierParams<'a>,
    pub(super) datagram_server: &'a str,
    pub(super) datagram_port: u16,
    pub(super) datagram_password: &'a str,
    pub(super) datagram_cipher: &'a str,
}

impl PacketPathManager {
    pub(super) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    pub(super) async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        params: &PacketPathChainParams<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let cipher_kind =
            shadowsocks::CipherKind::from_str(params.datagram_cipher).ok_or_else(|| {
                FlowFailure {
                    stage: "packet_path_cipher",
                    error: EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("unknown datagram cipher: {}", params.datagram_cipher),
                    )),
                    upstream: Some((params.datagram_server.to_owned(), params.datagram_port)),
                }
            })?;

        let entry = self
            .ensure_entry(proxy, params, cipher_kind)
            .await
            .map_err(|error| FlowFailure {
                stage: "packet_path_establish",
                error,
                upstream: Some(carrier_upstream(&params.carrier)),
            })?;

        let codec = shadowsocks::ShadowsocksDatagramCodec {
            cipher: cipher_kind,
            password: params.datagram_password.as_bytes().to_vec(),
        };
        let packet = codec
            .encode(packet_ref.target, packet_ref.port, packet_ref.payload)
            .map_err(|error| FlowFailure {
                stage: "packet_path_encode",
                error: error.into(),
                upstream: Some((params.datagram_server.to_owned(), params.datagram_port)),
            })?;

        let (response_tx, response_rx) = oneshot::channel();
        entry
            .waiters
            .lock()
            .expect("packet path waiters lock poisoned")
            .push_back(Waiter {
                target: packet_ref.target.clone(),
                port: packet_ref.port,
                tx: response_tx,
            });

        let datagram_target = Address::Domain(params.datagram_server.to_owned());
        if let Err(error) = entry
            .path
            .send_to(&datagram_target, params.datagram_port, &packet)
            .await
        {
            remove_waiter(&entry.waiters, packet_ref.target, packet_ref.port);
            return Err(FlowFailure {
                stage: "packet_path_send",
                error,
                upstream: Some((params.datagram_server.to_owned(), params.datagram_port)),
            });
        }

        ctx.chain_tasks.spawn(async move {
            match response_rx.await {
                Ok((target, port, payload)) => Ok((target, port, payload, Some(ctx.session_id))),
                Err(_) => Err(EngineError::Io(std::io::Error::other(
                    "packet path upstream closed",
                ))),
            }
        });

        Ok(packet_ref.payload.len())
    }

    async fn ensure_entry(
        &mut self,
        proxy: &Proxy,
        params: &PacketPathChainParams<'_>,
        cipher_kind: shadowsocks::CipherKind,
    ) -> Result<&Entry, EngineError> {
        let key = PathKey {
            carrier: carrier_key(&params.carrier),
            datagram_server: params.datagram_server.to_owned(),
            datagram_port: params.datagram_port,
            datagram_password: params.datagram_password.to_owned(),
            datagram_cipher: params.datagram_cipher.to_owned(),
        };

        if !self.upstreams.contains_key(&key) {
            let path = Arc::new(build_packet_path(proxy, &params.carrier).await?);
            let waiters = Arc::new(Mutex::new(VecDeque::new()));
            let codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>> =
                Arc::new(shadowsocks::ShadowsocksDatagramCodec {
                    cipher: cipher_kind,
                    password: params.datagram_password.as_bytes().to_vec(),
                });
            tokio::spawn(recv_loop(path.clone(), waiters.clone(), codec));
            self.upstreams.insert(key.clone(), Entry { path, waiters });
        }

        Ok(self
            .upstreams
            .get(&key)
            .expect("packet path entry inserted"))
    }
}

async fn build_packet_path(
    proxy: &Proxy,
    carrier: &PacketPathCarrierParams<'_>,
) -> Result<PacketPath, EngineError> {
    match carrier {
        #[cfg(feature = "socks5")]
        PacketPathCarrierParams::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } => {
            let association = Arc::new(
                ActiveUpstreamSocks5UdpAssociation::establish(
                    proxy,
                    tag,
                    server,
                    *port,
                    username.zip(*password),
                    0,
                )
                .await?,
            );
            Ok(PacketPath::Socks5(Socks5PacketPath { association }))
        }
        PacketPathCarrierParams::Shadowsocks {
            server,
            port,
            password,
            cipher,
            ..
        } => {
            let cipher_kind = shadowsocks::CipherKind::from_str(cipher).ok_or_else(|| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("unknown carrier cipher: {cipher}"),
                ))
            })?;
            let path =
                ShadowsocksPacketPath::establish(proxy, server, *port, password, cipher_kind)
                    .await?;
            Ok(PacketPath::Shadowsocks(path))
        }
        #[cfg(feature = "hysteria2")]
        PacketPathCarrierParams::Hysteria2 {
            server,
            port,
            password,
            client_fingerprint,
            ..
        } => {
            let path = Hysteria2PacketPath::establish(server, *port, password, *client_fingerprint)
                .await?;
            Ok(PacketPath::Hysteria2(path))
        }
    }
}

fn carrier_key(carrier: &PacketPathCarrierParams<'_>) -> CarrierKey {
    match carrier {
        #[cfg(feature = "socks5")]
        PacketPathCarrierParams::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } => CarrierKey::Socks5 {
            tag: (*tag).to_owned(),
            server: (*server).to_owned(),
            port: *port,
            username: username.map(ToOwned::to_owned),
            password: password.map(ToOwned::to_owned),
        },
        PacketPathCarrierParams::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } => CarrierKey::Shadowsocks {
            tag: (*tag).to_owned(),
            server: (*server).to_owned(),
            port: *port,
            password: (*password).to_owned(),
            cipher: (*cipher).to_owned(),
        },
        #[cfg(feature = "hysteria2")]
        PacketPathCarrierParams::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
        } => CarrierKey::Hysteria2 {
            tag: (*tag).to_owned(),
            server: (*server).to_owned(),
            port: *port,
            password: (*password).to_owned(),
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
        },
    }
}

fn carrier_upstream(carrier: &PacketPathCarrierParams<'_>) -> (String, u16) {
    match carrier {
        #[cfg(feature = "socks5")]
        PacketPathCarrierParams::Socks5 { server, port, .. } => ((*server).to_owned(), *port),
        PacketPathCarrierParams::Shadowsocks { server, port, .. } => ((*server).to_owned(), *port),
        #[cfg(feature = "hysteria2")]
        PacketPathCarrierParams::Hysteria2 { server, port, .. } => ((*server).to_owned(), *port),
    }
}

async fn recv_loop(
    path: Arc<PacketPath>,
    waiters: Arc<Mutex<VecDeque<Waiter>>>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = match path.recv_from(&mut buf).await {
            Ok(read) => read,
            Err(error) => {
                warn!(error = %error, "packet path recv loop stopped");
                break;
            }
        };
        let decoded = match codec.decode(&buf[..read]) {
            Some(d) => d,
            None => {
                warn!(bytes = read, "failed to decode inner datagram response");
                continue;
            }
        };
        debug!(
            target = ?decoded.0,
            port = decoded.1,
            bytes = decoded.2.len(),
            "decoded packet path datagram response"
        );
        if let Some(waiter) = remove_waiter(&waiters, &decoded.0, decoded.1) {
            let _ = waiter.tx.send(decoded);
        } else {
            warn!(
                target = ?decoded.0,
                port = decoded.1,
                "no waiter for packet path datagram response"
            );
        }
    }
}

fn remove_waiter(waiters: &Mutex<VecDeque<Waiter>>, target: &Address, port: u16) -> Option<Waiter> {
    let mut waiters = waiters.lock().expect("packet path waiters lock poisoned");
    let index = waiters
        .iter()
        .position(|waiter| waiter.target == *target && waiter.port == port)?;
    waiters.remove(index)
}
