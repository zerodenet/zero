//! Datagram-over-packet-path manager for UDP relay chains.
//!
//! Models the relay pattern where the first hop (carrier) provides a raw
//! send/recv channel ([`PacketPathCarrier`]) and the final hop (datagram)
//! encodes its protocol datagrams through that channel ([`DatagramCodec`]).
//!
//! The carrier and datagram roles are resolved via the adapter registry
//! ([`crate::protocol_adapter::ProtocolAdapter::udp_packet_path_carrier_descriptor`]
//! /
//! [`crate::protocol_adapter::ProtocolAdapter::udp_datagram_source`]); this
//! module never matches on `ResolvedLeafOutbound`. Adding a carrier = implement
//! [`PacketPathCarrier`] + the adapter's `build_udp_packet_path`; adding a
//! datagram = implement `DatagramCodec` + the adapter's `udp_datagram_source`.

use std::collections::{HashMap, VecDeque};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio::net::UdpSocket;
use tokio::sync::oneshot;
use tracing::{debug, warn};
use zero_core::Address;
use zero_engine::EngineError;

use super::packet_path_traits::{DatagramCodec, UdpFlowContext, UdpPacketRef};
use super::{FlowFailure, PacketPathCarrier};
use crate::runtime::Proxy;
use zero_engine::ResolvedLeafOutbound;

type RecvItem = (Address, u16, Vec<u8>);

// ── concrete carriers ─────────────────────────────────────────────────

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
            .direct_connector()
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

#[async_trait]
impl PacketPathCarrier for ShadowsocksPacketPath {
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
#[cfg(feature = "hysteria2")]
pub(super) struct Hysteria2PacketPath {
    conn: Arc<quinn::Connection>,
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
        let conn = Arc::new(connector.connect_raw().await?);
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
#[async_trait]
impl PacketPathCarrier for Hysteria2PacketPath {
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

// ── manager ───────────────────────────────────────────────────────────

struct Waiter {
    target: Address,
    port: u16,
    tx: oneshot::Sender<RecvItem>,
}

struct Entry {
    path: Arc<dyn PacketPathCarrier>,
    waiters: Arc<Mutex<VecDeque<Waiter>>>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    datagram_server: String,
    datagram_port: u16,
}

/// Owned, hashable identity of one carrier+datagram packet-path connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PathKey {
    /// Adapter-provided carrier identity (e.g. `"socks5|host:port|user"`).
    carrier_key: String,
    datagram_tag: String,
    datagram_server: String,
    datagram_port: u16,
    datagram_password: String,
    datagram_cipher: String,
}

pub(crate) struct PacketPathManager {
    upstreams: HashMap<PathKey, Entry>,
}

impl PacketPathManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }

    /// Start path: resolve carrier+datagram via the adapter registry, build on
    /// cache miss, encode + send. Takes the resolved leaves directly.
    pub(crate) async fn send(
        &mut self,
        ctx: UdpFlowContext<'_>,
        proxy: &Proxy,
        carrier_leaf: &ResolvedLeafOutbound<'_>,
        datagram_leaf: &ResolvedLeafOutbound<'_>,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let entry = self
            .ensure_entry(proxy, carrier_leaf, datagram_leaf)
            .await
            .map_err(|error| FlowFailure {
                stage: "packet_path_establish",
                error,
                upstream: Some(carrier_upstream(carrier_leaf)),
            })?;
        dispatch_via_entry(entry, ctx, packet_ref).await
    }

    /// Forward path: the carrier was cached at start time; look it up by the
    /// stored snapshot's cache key. No leaves available, so no re-dial.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn send_with_snapshot(
        &mut self,
        ctx: UdpFlowContext<'_>,
        carrier: &crate::runtime::udp_associate::sessions::UdpPacketPathCarrier,
        datagram_tag: &str,
        datagram_server: &str,
        datagram_port: u16,
        datagram_password: &str,
        datagram_cipher: &str,
        packet_ref: UdpPacketRef<'_>,
    ) -> Result<usize, FlowFailure> {
        let key = PathKey {
            carrier_key: carrier.cache_key().to_owned(),
            datagram_tag: datagram_tag.to_owned(),
            datagram_server: datagram_server.to_owned(),
            datagram_port,
            datagram_password: datagram_password.to_owned(),
            datagram_cipher: datagram_cipher.to_owned(),
        };
        let entry = self.upstreams.get(&key).ok_or_else(|| FlowFailure {
            stage: "packet_path_carrier_dropped",
            error: EngineError::Io(std::io::Error::other(
                "cached packet-path carrier not found",
            )),
            upstream: Some((datagram_server.to_owned(), datagram_port)),
        })?;
        dispatch_via_entry(entry, ctx, packet_ref).await
    }

    async fn ensure_entry(
        &mut self,
        proxy: &Proxy,
        carrier_leaf: &ResolvedLeafOutbound<'_>,
        datagram_leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<&Entry, EngineError> {
        let carrier_adapter = proxy.protocols.find_outbound_leaf(carrier_leaf)?;
        let datagram_adapter = proxy.protocols.find_outbound_leaf(datagram_leaf)?;
        let carrier_desc = carrier_adapter
            .udp_packet_path_carrier_descriptor(carrier_leaf)
            .ok_or_else(|| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "outbound does not support UDP packet-path carrier role",
                ))
            })?;
        let datagram = datagram_adapter
            .udp_datagram_source(datagram_leaf)
            .ok_or_else(|| {
                EngineError::Io(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "outbound does not support UDP packet-path datagram role",
                ))
            })?;

        debug!(
            carrier = %carrier_desc.cache_key,
            carrier_server = %carrier_desc.server,
            carrier_port = carrier_desc.port,
            datagram_tag = %datagram.tag,
            datagram_server = %datagram.server,
            datagram_port = datagram.port,
            "ensuring UDP packet-path relay chain"
        );

        let key = PathKey {
            carrier_key: carrier_desc.cache_key,
            datagram_tag: datagram.tag.to_owned(),
            datagram_server: datagram.server.to_owned(),
            datagram_port: datagram.port,
            datagram_password: datagram.password.to_owned(),
            datagram_cipher: datagram.cipher.to_owned(),
        };

        if !self.upstreams.contains_key(&key) {
            let path = carrier_adapter
                .build_udp_packet_path(proxy, carrier_leaf)
                .await?;
            let cipher_kind =
                shadowsocks::CipherKind::from_str(datagram.cipher).ok_or_else(|| {
                    EngineError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("unknown datagram cipher: {}", datagram.cipher),
                    ))
                })?;
            let codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>> =
                Arc::new(shadowsocks::ShadowsocksDatagramCodec {
                    cipher: cipher_kind,
                    password: datagram.password.as_bytes().to_vec(),
                });
            let waiters = Arc::new(Mutex::new(VecDeque::new()));
            tokio::spawn(recv_loop(path.clone(), waiters.clone(), codec.clone()));
            self.upstreams.insert(
                key.clone(),
                Entry {
                    path,
                    waiters,
                    codec,
                    datagram_server: datagram.server.to_owned(),
                    datagram_port: datagram.port,
                },
            );
        }

        Ok(self
            .upstreams
            .get(&key)
            .expect("packet path entry inserted"))
    }
}

/// Encode + send + spawn the response bridge. Shared by [`PacketPathManager::send`]
/// (start) and [`PacketPathManager::send_with_snapshot`] (forward).
async fn dispatch_via_entry(
    entry: &Entry,
    ctx: UdpFlowContext<'_>,
    packet_ref: UdpPacketRef<'_>,
) -> Result<usize, FlowFailure> {
    let codec = entry.codec.clone();
    let packet = codec
        .encode(packet_ref.target, packet_ref.port, packet_ref.payload)
        .map_err(|error| FlowFailure {
            stage: "packet_path_encode",
            error: error.into(),
            upstream: Some((entry.datagram_server.clone(), entry.datagram_port)),
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

    let datagram_target = Address::Domain(entry.datagram_server.clone());
    let datagram_port = entry.datagram_port;
    if let Err(error) = entry
        .path
        .send_to(&datagram_target, datagram_port, &packet)
        .await
    {
        remove_waiter(&entry.waiters, packet_ref.target, packet_ref.port);
        return Err(FlowFailure {
            stage: "packet_path_send",
            error,
            upstream: Some((entry.datagram_server.clone(), entry.datagram_port)),
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

/// `(server, port)` of a carrier leaf, for diagnostics.
fn carrier_upstream(leaf: &ResolvedLeafOutbound<'_>) -> (String, u16) {
    use crate::runtime::orchestration::endpoint;
    endpoint(leaf)
        .map(|e| (e.server.to_owned(), e.port))
        .unwrap_or_default()
}

// ── per-carrier constructors (called by adapters) ─────────────────────

/// Build a Shadowsocks packet-path carrier (raw UDP socket to the SS server).
pub(crate) async fn build_shadowsocks_packet_path(
    proxy: &Proxy,
    server: &str,
    port: u16,
    password: &str,
    cipher: &str,
) -> Result<Arc<dyn PacketPathCarrier>, EngineError> {
    let cipher_kind = shadowsocks::CipherKind::from_str(cipher).ok_or_else(|| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("unknown carrier cipher: {cipher}"),
        ))
    })?;
    let path = ShadowsocksPacketPath::establish(proxy, server, port, password, cipher_kind).await?;
    Ok(Arc::new(path))
}

/// Build a Hysteria2 packet-path carrier (QUIC datagrams to the H2 server).
#[cfg(feature = "hysteria2")]
pub(crate) async fn build_hysteria2_packet_path(
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Result<Arc<dyn PacketPathCarrier>, EngineError> {
    let path = Hysteria2PacketPath::establish(server, port, password, client_fingerprint).await?;
    Ok(Arc::new(path))
}

async fn recv_loop(
    path: Arc<dyn PacketPathCarrier>,
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
