// Hysteria2 UDP datagram — udp.rs

use alloc::borrow::ToOwned;
#[cfg(feature = "tokio")]
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zero_core::{Address, Error, UdpFlowPacket};
use zero_traits::DatagramCodec;

#[cfg(feature = "tokio")]
use alloc::sync::Arc;
#[cfg(feature = "tokio")]
use tokio::sync::{broadcast, mpsc};
#[cfg(all(feature = "tokio", feature = "crypto"))]
use zero_traits::AsyncSocket;

/// One plaintext UDP payload to encode into a Hysteria2 UDP datagram.
#[derive(Debug, Clone, Copy)]
pub struct Hysteria2UdpPacketTarget<'a> {
    pub session_id: u16,
    pub packet_id: u16,
    pub target: &'a Address,
    pub port: u16,
    pub payload: &'a [u8],
}

/// Parsed Hysteria2 UDP datagram.
#[derive(Debug, Clone)]
pub struct Hysteria2UdpPacket {
    pub session_id: u16,
    pub packet_id: u16,
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

/// Protocol-owned decoded inbound UDP request.
pub struct Hysteria2InboundUdpRequest {
    session_id: u16,
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl Hysteria2InboundUdpRequest {
    fn from_packet(packet: Hysteria2UdpPacket) -> Self {
        Self {
            session_id: packet.session_id,
            target: packet.target,
            port: packet.port,
            payload: packet.payload,
        }
    }

    pub fn target(&self) -> &Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

/// Stateful inbound UDP bridge for Hysteria2 datagram sessions.
#[cfg(feature = "tokio")]
pub struct Hysteria2InboundUdpSession {
    h2_sessions_by_proxy_session: BTreeMap<u64, u16>,
}

#[cfg(feature = "tokio")]
impl Hysteria2InboundUdpSession {
    pub fn new() -> Self {
        Self {
            h2_sessions_by_proxy_session: BTreeMap::new(),
        }
    }

    pub fn decode_request(&self, data: &[u8]) -> Result<Hysteria2InboundUdpRequest, Error> {
        Hysteria2InboundUdpCodec
            .decode_datagram(data)
            .map(Hysteria2InboundUdpRequest::from_packet)
    }

    pub fn record_proxy_session(
        &mut self,
        proxy_session_id: u64,
        request: &Hysteria2InboundUdpRequest,
    ) {
        self.h2_sessions_by_proxy_session
            .insert(proxy_session_id, request.session_id);
    }

    pub fn send_response(
        &self,
        conn: &quinn::Connection,
        proxy_session_id: u64,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<usize>, Error> {
        let Some(&h2_session_id) = self.h2_sessions_by_proxy_session.get(&proxy_session_id) else {
            return Ok(None);
        };
        Hysteria2InboundUdpCodec
            .send_datagram(conn, h2_session_id, target, port, payload)
            .map(Some)
    }
}

#[cfg(feature = "tokio")]
impl Default for Hysteria2InboundUdpSession {
    fn default() -> Self {
        Self::new()
    }
}

/// Build a Hysteria2 UDP datagram.
/// Format: [session_id:2][pkt_id:2][addr_type:1][addr:var][port:2][payload:var]
pub fn build_udp_datagram(
    session_id: u16,
    packet_id: u16,
    address: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let addr_bytes = crate::shared::encode_address(address)?;
    let mut buf = Vec::with_capacity(4 + addr_bytes.len() + 2 + payload.len());
    buf.extend_from_slice(&session_id.to_be_bytes());
    buf.extend_from_slice(&packet_id.to_be_bytes());
    buf.extend_from_slice(&addr_bytes);
    buf.extend_from_slice(&port.to_be_bytes());
    buf.extend_from_slice(payload);
    Ok(buf)
}

/// Parse a Hysteria2 UDP datagram.
pub fn parse_udp_datagram(data: &[u8]) -> Result<Hysteria2UdpPacket, Error> {
    if data.len() < 5 {
        return Err(Error::Protocol("hysteria2: truncated UDP datagram"));
    }
    let session_id = u16::from_be_bytes([data[0], data[1]]);
    let packet_id = u16::from_be_bytes([data[2], data[3]]);
    let addr_type = data[4];
    let (target, addr_end) = match addr_type {
        crate::shared::ADDR_TYPE_IPV4 => {
            if data.len() < 9 {
                return Err(Error::Protocol("hysteria2: truncated IPv4 in datagram"));
            }
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[5..9]);
            (Address::Ipv4(bytes), 9)
        }
        crate::shared::ADDR_TYPE_IPV6 => {
            if data.len() < 21 {
                return Err(Error::Protocol("hysteria2: truncated IPv6 in datagram"));
            }
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&data[5..21]);
            (Address::Ipv6(bytes), 21)
        }
        crate::shared::ADDR_TYPE_DOMAIN => {
            if data.len() < 6 {
                return Err(Error::Protocol("hysteria2: truncated domain in datagram"));
            }
            let len = data[5] as usize;
            if data.len() < 6 + len + 2 {
                return Err(Error::Protocol(
                    "hysteria2: truncated domain payload in datagram",
                ));
            }
            let domain = String::from_utf8(data[6..6 + len].to_vec())
                .map_err(|_| Error::Protocol("hysteria2: invalid domain UTF-8"))?;
            (Address::Domain(domain), 6 + len)
        }
        _ => {
            return Err(Error::Unsupported(
                "hysteria2: unknown address type in datagram",
            ))
        }
    };
    if data.len() < addr_end + 2 {
        return Err(Error::Protocol("hysteria2: truncated port in datagram"));
    }
    let port = u16::from_be_bytes([data[addr_end], data[addr_end + 1]]);
    let payload = data[addr_end + 2..].to_vec();

    Ok(Hysteria2UdpPacket {
        session_id,
        packet_id,
        target,
        port,
        payload,
    })
}

pub fn encode_udp_flow_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    build_udp_datagram(0, 0, target, port, payload)
}

pub fn decode_udp_flow_packet(data: &[u8]) -> Result<Hysteria2UdpPacket, Error> {
    parse_udp_datagram(data)
}

pub fn decode_inbound_udp_datagram(data: &[u8]) -> Result<Hysteria2UdpPacket, Error> {
    parse_udp_datagram(data)
}

pub fn encode_inbound_udp_datagram(
    session_id: u16,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    build_udp_datagram(session_id, 0, target, port, payload)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Hysteria2InboundUdpCodec;

impl Hysteria2InboundUdpCodec {
    pub fn decode_datagram(&self, data: &[u8]) -> Result<Hysteria2UdpPacket, Error> {
        decode_inbound_udp_datagram(data)
    }

    pub fn encode_datagram(
        &self,
        session_id: u16,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_inbound_udp_datagram(session_id, target, port, payload)
    }

    #[cfg(feature = "tokio")]
    pub fn send_datagram(
        &self,
        conn: &quinn::Connection,
        session_id: u16,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        let datagram = self.encode_datagram(session_id, target, port, payload)?;
        let len = datagram.len();
        conn.send_datagram(datagram.into())
            .map_err(|_| Error::Io("failed to send Hysteria2 UDP datagram"))?;
        Ok(len)
    }
}

fn udp_cache_key(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> String {
    let fingerprint = client_fingerprint
        .map(|value| alloc::format!("|fp:{value}"))
        .unwrap_or_default();
    alloc::format!("hysteria2|{tag}|{server}:{port}|{password}{fingerprint}")
}

pub struct Hysteria2UdpPacketPathConfig<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    password: &'a str,
    client_fingerprint: Option<&'a str>,
}

impl<'a> Hysteria2UdpPacketPathConfig<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        client_fingerprint: Option<&'a str>,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            password,
            client_fingerprint,
        }
    }

    pub fn cache_key(&self) -> String {
        udp_cache_key(
            self.tag,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
    }

    pub fn packet_path_cache_key(&self) -> String {
        self.cache_key()
    }

    pub fn packet_path_codec(&self) -> impl DatagramCodec<Address, Error = Error> {
        self.codec()
    }

    pub fn flow_resume(&self) -> Hysteria2UdpFlowResume {
        Hysteria2UdpFlowResume::new(self.password, self.client_fingerprint)
    }

    pub fn connector_profile(&self) -> Hysteria2UdpConnectorProfile {
        self.flow_resume().connector_profile()
    }

    pub fn codec(&self) -> impl DatagramCodec<Address, Error = Error> {
        udp_flow_codec()
    }
}

/// Codec state for a Hysteria2 UDP datagram chain hop.
///
/// Hysteria2 UDP flow framing has no negotiated per-flow crypto state once the
/// QUIC connection is established, so this codec is stateless.
#[derive(Debug, Default, Clone, Copy)]
pub struct Hysteria2DatagramCodec;

pub fn udp_flow_codec() -> impl DatagramCodec<Address, Error = Error> {
    Hysteria2DatagramCodec
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2UdpFlowPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

impl Hysteria2UdpFlowPacket {
    pub fn new(target: Address, port: u16, payload: Vec<u8>) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub fn from_parts(target: &Address, port: u16, payload: &[u8]) -> Self {
        Self::new(target.clone(), port, payload.to_vec())
    }

    pub fn encode_with(&self, resume: &Hysteria2UdpFlowResume) -> Result<Vec<u8>, Error> {
        resume.encode_packet(&self.target, self.port, &self.payload)
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}

pub fn udp_flow_packet(target: &Address, port: u16, payload: &[u8]) -> Hysteria2UdpFlowPacket {
    Hysteria2UdpFlowPacket::from_parts(target, port, payload)
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Hysteria2UdpFlowIo;

impl Hysteria2UdpFlowIo {
    pub fn encode_packet(&self, packet: &UdpFlowPacket) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(&packet.target, packet.port, &packet.payload)
    }

    pub fn encode_initial_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    pub fn decode_packet(&self, data: &[u8]) -> Option<UdpFlowPacket> {
        let decoded = decode_udp_flow_packet(data).ok()?;
        Some(UdpFlowPacket::new(
            decoded.target,
            decoded.port,
            decoded.payload,
        ))
    }
}

#[cfg(feature = "tokio")]
pub type Hysteria2UdpFlowResponse = (Address, u16, Vec<u8>);

#[cfg(feature = "tokio")]
type Hysteria2UdpFlowResponses = broadcast::Sender<Hysteria2UdpFlowResponse>;

#[cfg(feature = "tokio")]
pub type Hysteria2UdpFlowResponseReceiver = broadcast::Receiver<Hysteria2UdpFlowResponse>;

#[cfg(feature = "tokio")]
#[derive(Clone)]
struct Hysteria2UdpFlowSender {
    send_tx: mpsc::Sender<UdpFlowPacket>,
}

#[cfg(feature = "tokio")]
#[derive(Clone)]
pub struct Hysteria2InitialUdpFlowPacket {
    packet: UdpFlowPacket,
}

#[cfg(feature = "tokio")]
impl Hysteria2InitialUdpFlowPacket {
    pub fn from_parts(target: &Address, port: u16, payload: &[u8]) -> Self {
        Self {
            packet: UdpFlowPacket::from_parts(target, port, payload),
        }
    }
}

#[cfg(feature = "tokio")]
pub struct Hysteria2UdpFlowHandle {
    sender: Hysteria2UdpFlowSender,
    responses: Hysteria2UdpFlowResponses,
}

#[cfg(feature = "tokio")]
#[derive(Clone)]
pub struct Hysteria2UdpFlowSession {
    sender: Hysteria2UdpFlowSender,
    responses: Hysteria2UdpFlowResponses,
}

#[cfg(feature = "tokio")]
impl Hysteria2UdpFlowSession {
    pub fn new(handle: Hysteria2UdpFlowHandle) -> Self {
        Self {
            sender: handle.sender,
            responses: handle.responses,
        }
    }

    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        self.sender.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> Hysteria2UdpFlowResponseReceiver {
        self.responses.subscribe()
    }
}

#[cfg(feature = "tokio")]
#[derive(Clone)]
pub struct Hysteria2UdpFlowConnection {
    session: Hysteria2UdpFlowSession,
}

#[cfg(feature = "tokio")]
impl Hysteria2UdpFlowConnection {
    pub fn new(session: Hysteria2UdpFlowSession) -> Self {
        Self { session }
    }

    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        self.session.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> Hysteria2UdpFlowResponseReceiver {
        self.session.subscribe_responses()
    }
}

#[cfg(feature = "tokio")]
impl Hysteria2UdpFlowSender {
    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        let packet = UdpFlowPacket::from_parts(target, port, payload);
        let packet_len = packet.payload.len();
        self.send_tx
            .send(packet)
            .await
            .map_err(|_| Error::Io("hysteria2 udp flow closed"))?;
        Ok(packet_len)
    }
}

#[cfg(feature = "tokio")]
pub fn spawn_udp_flow(
    conn: Arc<quinn::Connection>,
    initial_packet: Hysteria2InitialUdpFlowPacket,
    flow_io: Hysteria2UdpFlowIo,
) -> Hysteria2UdpFlowHandle {
    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (responses, _) = broadcast::channel::<Hysteria2UdpFlowResponse>(32);

    spawn_send_task(conn.clone(), initial_packet, flow_io, send_rx);
    spawn_recv_task(conn, flow_io, responses.clone());

    Hysteria2UdpFlowHandle {
        sender: Hysteria2UdpFlowSender { send_tx },
        responses,
    }
}

#[cfg(feature = "tokio")]
pub fn start_udp_flow_with_initial_packet(
    conn: Arc<quinn::Connection>,
    target: &Address,
    port: u16,
    payload: &[u8],
    resume: Hysteria2UdpFlowResume,
) -> Hysteria2UdpFlowConnection {
    let flow_io = resume.flow_io();
    let initial_packet = Hysteria2InitialUdpFlowPacket::from_parts(target, port, payload);
    Hysteria2UdpFlowConnection::new(Hysteria2UdpFlowSession::new(spawn_udp_flow(
        conn,
        initial_packet,
        flow_io,
    )))
}

#[cfg(feature = "tokio")]
fn spawn_send_task(
    conn: Arc<quinn::Connection>,
    initial_packet: Hysteria2InitialUdpFlowPacket,
    flow_io: Hysteria2UdpFlowIo,
    mut send_rx: mpsc::Receiver<UdpFlowPacket>,
) {
    tokio::spawn(async move {
        if let Ok(datagram) = flow_io.encode_packet(&initial_packet.packet) {
            if conn.send_datagram(datagram.into()).is_err() {
                return;
            }
        }
        while let Some(packet) = send_rx.recv().await {
            let Ok(datagram) = flow_io.encode_packet(&packet) else {
                break;
            };
            if conn.send_datagram(datagram.into()).is_err() {
                break;
            }
        }
    });
}

#[cfg(feature = "tokio")]
fn spawn_recv_task(
    conn: Arc<quinn::Connection>,
    flow_io: Hysteria2UdpFlowIo,
    responses: Hysteria2UdpFlowResponses,
) {
    tokio::spawn(async move {
        while let Ok(data) = conn.read_datagram().await {
            let Some(packet) = flow_io.decode_packet(&data) else {
                continue;
            };
            if responses
                .send((packet.target, packet.port, packet.payload))
                .is_err()
            {
                break;
            }
        }
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2UdpFlowResume {
    password: String,
    client_fingerprint: Option<String>,
}

impl Hysteria2UdpFlowResume {
    pub fn new(password: &str, client_fingerprint: Option<&str>) -> Self {
        Self {
            password: password.to_owned(),
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
        }
    }

    fn peer_config(&self) -> Hysteria2UdpPeerConfig<'_> {
        Hysteria2UdpPeerConfig {
            password: &self.password,
        }
    }

    fn leaf_cache_key(&self, server: &str, port: u16) -> Hysteria2UdpLeafKey {
        self.peer_config().leaf_cache_key(server, port)
    }

    fn flow_key(&self, server: &str, port: u16) -> Hysteria2UdpFlowKey {
        Hysteria2UdpFlowKey::Leaf(self.leaf_cache_key(server, port))
    }

    fn cache_key(&self, server: &str, port: u16) -> Hysteria2UdpCacheKey {
        Hysteria2UdpCacheKey::from_flow_key(self.flow_key(server, port))
    }

    pub fn flow_cache_key(&self, server: &str, port: u16) -> String {
        alloc::format!(
            "leaf|{server}:{port}|password:{}",
            self.peer_config().password
        )
    }

    pub fn connector_profile(&self) -> Hysteria2UdpConnectorProfile {
        Hysteria2UdpConnectorProfile {
            password: self.password.clone(),
            client_fingerprint: self.client_fingerprint.clone(),
        }
    }

    pub fn codec(&self) -> impl DatagramCodec<Address, Error = Error> {
        udp_flow_codec()
    }

    pub fn flow_io(&self) -> Hysteria2UdpFlowIo {
        Hysteria2UdpFlowIo
    }

    pub fn encode_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    pub fn encode_flow_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.encode_packet(target, port, payload)
    }

    pub fn decode_packet(&self, data: &[u8]) -> Option<(Address, u16, Vec<u8>)> {
        let decoded = decode_udp_flow_packet(data).ok()?;
        Some((decoded.target, decoded.port, decoded.payload))
    }

    pub fn decode_flow_packet(&self, data: &[u8]) -> Option<Hysteria2UdpFlowPacket> {
        let (target, port, payload) = self.decode_packet(data)?;
        Some(Hysteria2UdpFlowPacket::new(target, port, payload))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Hysteria2UdpFlowKey {
    Leaf(Hysteria2UdpLeafKey),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Hysteria2UdpCacheKey(Hysteria2UdpLeafKey);

impl Hysteria2UdpCacheKey {
    fn from_flow_key(flow_key: Hysteria2UdpFlowKey) -> Self {
        match flow_key {
            Hysteria2UdpFlowKey::Leaf(leaf_key) => Self(leaf_key),
        }
    }
}

pub struct Hysteria2UdpFlowStore<T> {
    entries: alloc::collections::BTreeMap<Hysteria2UdpCacheKey, T>,
}

impl<T> Default for Hysteria2UdpFlowStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Hysteria2UdpFlowStore<T> {
    pub fn new() -> Self {
        Self {
            entries: alloc::collections::BTreeMap::new(),
        }
    }

    pub fn get(&self, resume: &Hysteria2UdpFlowResume, server: &str, port: u16) -> Option<&T> {
        let key = resume.cache_key(server, port);
        self.entries.get(&key)
    }

    pub fn insert(
        &mut self,
        resume: &Hysteria2UdpFlowResume,
        server: &str,
        port: u16,
        value: T,
    ) -> Option<T> {
        let key = resume.cache_key(server, port);
        self.entries.insert(key, value)
    }
}

#[cfg(feature = "tokio")]
#[derive(Default)]
pub struct Hysteria2UdpFlowSessions {
    entries: Hysteria2UdpFlowStore<Hysteria2UdpFlowConnection>,
}

#[cfg(feature = "tokio")]
impl Hysteria2UdpFlowSessions {
    pub fn new() -> Self {
        Self {
            entries: Hysteria2UdpFlowStore::new(),
        }
    }

    pub fn get(
        &self,
        resume: &Hysteria2UdpFlowResume,
        server: &str,
        port: u16,
    ) -> Option<&Hysteria2UdpFlowConnection> {
        self.entries.get(resume, server, port)
    }

    pub fn insert(
        &mut self,
        resume: &Hysteria2UdpFlowResume,
        server: &str,
        port: u16,
        connection: Hysteria2UdpFlowConnection,
    ) -> Option<Hysteria2UdpFlowConnection> {
        self.entries.insert(resume, server, port, connection)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2UdpConnectorProfile {
    password: String,
    client_fingerprint: Option<String>,
}

impl Hysteria2UdpConnectorProfile {
    pub fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }

    #[cfg(all(feature = "tokio", feature = "crypto"))]
    pub async fn authenticate_connection<S>(
        &self,
        conn: &quinn::Connection,
        stream: &mut S,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let mut salt = [0u8; 32];
        conn.export_keying_material(&mut salt, b"hysteria2 auth", &[])
            .map_err(|_| Error::Io("hysteria2 key export failed"))?;

        crate::Hysteria2Outbound
            .authenticate_with_salt(stream, &self.password, &salt)
            .await
    }
}

#[derive(Debug, Clone, Copy)]
struct Hysteria2UdpPeerConfig<'a> {
    password: &'a str,
}

impl<'a> Hysteria2UdpPeerConfig<'a> {
    fn leaf_cache_key(&self, server: &str, port: u16) -> Hysteria2UdpLeafKey {
        Hysteria2UdpLeafKey {
            server: server.to_owned(),
            port,
            password: self.password.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Hysteria2UdpLeafKey {
    server: String,
    port: u16,
    password: String,
}

impl DatagramCodec<Address> for Hysteria2DatagramCodec {
    type Error = Error;

    fn encode(&self, target: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, Self::Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    fn decode(&self, data: &[u8]) -> Option<(Address, u16, Vec<u8>)> {
        let decoded = decode_udp_flow_packet(data).ok()?;
        Some((decoded.target, decoded.port, decoded.payload))
    }
}
