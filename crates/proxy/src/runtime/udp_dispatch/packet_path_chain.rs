use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use tokio::sync::oneshot;
use zero_core::Address;
use zero_engine::EngineError;

use super::{DatagramCodec, FlowFailure, UdpFlowContext, UdpPacketPath, UdpPacketRef};
use crate::outbound::socks5::ActiveUpstreamSocks5UdpAssociation;
use crate::runtime::Proxy;

type RecvItem = (Address, u16, Vec<u8>);

// Packet path: SOCKS5 UDP ASSOCIATE.

pub(super) struct Socks5PacketPath {
    association: Arc<ActiveUpstreamSocks5UdpAssociation>,
}

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
            .map_err(|e| EngineError::Io(std::io::Error::other(e.to_string())))?;
        let len = packet.payload.len();
        buf[..len].copy_from_slice(&packet.payload);
        Ok(len)
    }
}

// Manager.

struct Waiter {
    target: Address,
    port: u16,
    tx: oneshot::Sender<RecvItem>,
}

struct Entry {
    path: Arc<Socks5PacketPath>,
    waiters: Arc<Mutex<VecDeque<Waiter>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PathKey {
    carrier_tag: String,
    carrier_server: String,
    carrier_port: u16,
    carrier_username: Option<String>,
    carrier_password: Option<String>,
    datagram_server: String,
    datagram_port: u16,
    datagram_password: String,
    datagram_cipher: String,
}

pub(super) struct PacketPathManager {
    upstreams: HashMap<PathKey, Entry>,
}

/// Resolved parameters for a datagram-over-packet-path relay chain.
///
/// Produced by [`super::resolve_udp_packet_path_chain`] from a resolved
/// outbound chain. Contains both the carrier (packet path) parameters and
/// the inner datagram protocol parameters.
pub(super) struct PacketPathChainParams<'a> {
    pub(super) datagram_tag: &'a str,
    pub(super) carrier_tag: &'a str,
    pub(super) carrier_server: &'a str,
    pub(super) carrier_port: u16,
    pub(super) carrier_username: Option<&'a str>,
    pub(super) carrier_password: Option<&'a str>,
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
                upstream: Some((params.carrier_server.to_owned(), params.carrier_port)),
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
            carrier_tag: params.carrier_tag.to_owned(),
            carrier_server: params.carrier_server.to_owned(),
            carrier_port: params.carrier_port,
            carrier_username: params.carrier_username.map(ToOwned::to_owned),
            carrier_password: params.carrier_password.map(ToOwned::to_owned),
            datagram_server: params.datagram_server.to_owned(),
            datagram_port: params.datagram_port,
            datagram_password: params.datagram_password.to_owned(),
            datagram_cipher: params.datagram_cipher.to_owned(),
        };

        if !self.upstreams.contains_key(&key) {
            let association = Arc::new(
                ActiveUpstreamSocks5UdpAssociation::establish(
                    proxy,
                    params.carrier_tag,
                    params.carrier_server,
                    params.carrier_port,
                    params.carrier_username.zip(params.carrier_password),
                    0,
                )
                .await?,
            );
            let path = Arc::new(Socks5PacketPath { association });
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

async fn recv_loop(
    path: Arc<Socks5PacketPath>,
    waiters: Arc<Mutex<VecDeque<Waiter>>>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) {
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let read = match path.recv_from(&mut buf).await {
            Ok(read) => read,
            Err(_) => break,
        };
        let decoded = match codec.decode(&buf[..read]) {
            Some(d) => d,
            None => continue,
        };
        if let Some(waiter) = remove_waiter(&waiters, &decoded.0, decoded.1) {
            let _ = waiter.tx.send(decoded);
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
