// Hysteria2 UDP datagram — udp.rs

use alloc::string::String;
use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::DatagramCodec;

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
}

pub fn udp_cache_key(
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
    pub tag: &'a str,
    pub server: &'a str,
    pub port: u16,
    pub password: &'a str,
    pub client_fingerprint: Option<&'a str>,
}

impl Hysteria2UdpPacketPathConfig<'_> {
    pub fn cache_key(&self) -> String {
        udp_cache_key(
            self.tag,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
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

pub type Hysteria2UdpFlowResponse = (Address, u16, Vec<u8>);

#[cfg(feature = "runtime")]
pub trait Hysteria2UdpDatagramIo: Send + Sync + Clone + 'static {
    fn send_datagram(
        &self,
        datagram: Vec<u8>,
    ) -> impl core::future::Future<Output = Result<(), Error>> + Send;

    fn read_datagram(&self) -> impl core::future::Future<Output = Result<Vec<u8>, Error>> + Send;
}

#[cfg(feature = "runtime")]
#[derive(Clone)]
pub struct Hysteria2UdpFlowSender {
    send_tx: tokio::sync::mpsc::Sender<Hysteria2UdpFlowPacket>,
}

#[cfg(feature = "runtime")]
impl Hysteria2UdpFlowSender {
    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        let packet = udp_flow_packet(target, port, payload);
        let packet_len = packet.payload.len();
        self.send_tx
            .send(packet)
            .await
            .map_err(|_| Error::Io("hysteria2 udp flow closed"))?;
        Ok(packet_len)
    }
}

#[cfg(feature = "runtime")]
pub struct Hysteria2UdpFlowHandle {
    pub sender: Hysteria2UdpFlowSender,
    pub responses: tokio::sync::broadcast::Sender<Hysteria2UdpFlowResponse>,
}

#[cfg(feature = "runtime")]
pub fn open_udp_flow<I>(
    io: I,
    initial_packet: Hysteria2UdpFlowPacket,
    resume: Hysteria2UdpFlowResume,
) -> Hysteria2UdpFlowHandle
where
    I: Hysteria2UdpDatagramIo,
{
    let (send_tx, send_rx) = tokio::sync::mpsc::channel::<Hysteria2UdpFlowPacket>(32);
    let (responses, _) = tokio::sync::broadcast::channel::<Hysteria2UdpFlowResponse>(32);
    spawn_udp_flow_send_task(io.clone(), initial_packet, resume.clone(), send_rx);
    spawn_udp_flow_recv_task(io, resume, responses.clone());
    Hysteria2UdpFlowHandle {
        sender: Hysteria2UdpFlowSender { send_tx },
        responses,
    }
}

#[cfg(feature = "runtime")]
fn spawn_udp_flow_send_task<I>(
    io: I,
    initial_packet: Hysteria2UdpFlowPacket,
    resume: Hysteria2UdpFlowResume,
    mut send_rx: tokio::sync::mpsc::Receiver<Hysteria2UdpFlowPacket>,
) where
    I: Hysteria2UdpDatagramIo,
{
    tokio::spawn(async move {
        if let Ok(datagram) = initial_packet.encode_with(&resume) {
            if io.send_datagram(datagram).await.is_err() {
                return;
            }
        }
        while let Some(packet) = send_rx.recv().await {
            let Ok(datagram) = packet.encode_with(&resume) else {
                break;
            };
            if io.send_datagram(datagram).await.is_err() {
                break;
            }
        }
    });
}

#[cfg(feature = "runtime")]
fn spawn_udp_flow_recv_task<I>(
    io: I,
    resume: Hysteria2UdpFlowResume,
    responses: tokio::sync::broadcast::Sender<Hysteria2UdpFlowResponse>,
) where
    I: Hysteria2UdpDatagramIo,
{
    tokio::spawn(async move {
        while let Ok(data) = io.read_datagram().await {
            let Some(packet) = resume.decode_flow_packet(&data) else {
                continue;
            };
            if responses.send(packet.into_parts()).is_err() {
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

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }

    pub fn peer_config(&self) -> Hysteria2UdpPeerConfig<'_> {
        Hysteria2UdpPeerConfig {
            password: &self.password,
            client_fingerprint: self.client_fingerprint.as_deref(),
        }
    }

    pub fn leaf_cache_key(&self, server: &str, port: u16) -> Hysteria2UdpLeafKey {
        self.peer_config().leaf_cache_key(server, port)
    }

    pub fn flow_key(&self, server: &str, port: u16) -> Hysteria2UdpFlowKey {
        Hysteria2UdpFlowKey::Leaf(self.leaf_cache_key(server, port))
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

    pub fn encode_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(target, port, payload)
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Hysteria2UdpFlowKey {
    Leaf(Hysteria2UdpLeafKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2UdpConnectorProfile {
    password: String,
    client_fingerprint: Option<String>,
}

impl Hysteria2UdpConnectorProfile {
    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Hysteria2UdpPeerConfig<'a> {
    password: &'a str,
    client_fingerprint: Option<&'a str>,
}

impl<'a> Hysteria2UdpPeerConfig<'a> {
    pub fn password(&self) -> &'a str {
        self.password
    }

    pub fn client_fingerprint(&self) -> Option<&'a str> {
        self.client_fingerprint
    }

    pub fn leaf_cache_key(&self, server: &str, port: u16) -> Hysteria2UdpLeafKey {
        Hysteria2UdpLeafKey {
            server: server.to_owned(),
            port,
            password: self.password.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hysteria2UdpLeafKey {
    server: String,
    port: u16,
    password: String,
}

impl Hysteria2UdpLeafKey {
    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }
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
