// Mieru UDP associate encapsulation — udp.rs
//
// SOCKS5 UDP ASSOCIATE over mieru wraps datagrams with markers:
//   [0x00] [len: 2 bytes BE] [data: len bytes] [0xff]
//
// This preserves datagram boundaries when transmitted over TCP streams.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "crypto")]
use std::collections::HashMap;
#[cfg(feature = "crypto")]
use std::net::SocketAddr;

use zero_core::{Address, Error};
use zero_traits::DatagramCodec;

#[cfg(feature = "crypto")]
pub use crate::outbound::{
    establish_udp_flow_with_resume, spawn_udp_flow, MieruUdpFlowConnection, MieruUdpFlowHandle,
    MieruUdpFlowIo, MieruUdpFlowPacket, MieruUdpFlowResponse, MieruUdpFlowResponseReceiver,
    MieruUdpFlowSession,
};

const ATYP_IPV4: u8 = 0x01;
const ATYP_DOMAIN: u8 = 0x03;
const ATYP_IPV6: u8 = 0x04;

/// One raw UDP datagram to wrap for Mieru UDP associate.
#[derive(Debug, Clone, Copy)]
pub struct MieruUdpAssociatePacket<'a> {
    pub payload: &'a [u8],
}

/// One unwrapped Mieru UDP associate payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MieruUdpAssociatePayload {
    payload: Vec<u8>,
}

impl MieruUdpAssociatePayload {
    pub fn new(payload: Vec<u8>) -> Self {
        Self { payload }
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MieruInboundUdpPacket {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl MieruInboundUdpPacket {
    pub fn new(target: Address, port: u16, payload: Vec<u8>) -> Self {
        Self {
            target,
            port,
            payload,
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

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MieruInboundUdpRequest {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl MieruInboundUdpRequest {
    fn from_packet(packet: MieruInboundUdpPacket) -> Self {
        let (target, port, payload) = packet.into_parts();
        Self {
            target,
            port,
            payload,
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

    pub fn target_endpoint(&self) -> (&Address, u16) {
        (&self.target, self.port)
    }

    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }
}

#[cfg(feature = "crypto")]
#[derive(Debug, Default)]
pub struct MieruInboundUdpSession {
    targets_by_sender: HashMap<SocketAddr, (Address, u16)>,
}

#[cfg(feature = "crypto")]
impl MieruInboundUdpSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn decode_request(&self, data: &[u8]) -> Result<MieruInboundUdpRequest, Error> {
        MieruUdpFlowCodec
            .decode_packet(data)
            .map(MieruInboundUdpRequest::from_packet)
    }

    pub fn record_target(&mut self, sender: SocketAddr, target: Address, port: u16) {
        self.targets_by_sender.insert(sender, (target, port));
    }

    pub async fn write_response_tokio<W>(
        &self,
        writer: &mut W,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<Option<usize>, Error>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let Some((target, port)) = self.targets_by_sender.get(&sender) else {
            return Ok(None);
        };
        MieruUdpFlowCodec
            .write_response_tokio(writer, target, *port, payload)
            .await
            .map(Some)
    }
}

/// Wrap a raw UDP datagram for transmission through mieru TCP/UDP proxy.
///
/// Format: 0x00 || data_length(u16 BE) || data || 0xff
pub(crate) fn wrap_udp_associate(data: &[u8]) -> Vec<u8> {
    let len = data.len() as u16;
    let mut buf = Vec::with_capacity(1 + 2 + data.len() + 1);
    buf.push(0x00);
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(data);
    buf.push(0xff);
    buf
}

/// Unwrap a mieru UDP associate datagram back into the original UDP payload.
///
/// Returns the raw datagram bytes.
pub(crate) fn unwrap_udp_associate(data: &[u8]) -> Result<Vec<u8>, Error> {
    if data.len() < 4 {
        return Err(Error::Protocol("mieru udp: too short"));
    }
    if data[0] != 0x00 {
        return Err(Error::Protocol("mieru udp: missing start marker"));
    }

    let data_len = u16::from_be_bytes([data[1], data[2]]) as usize;
    if data.len() < 3 + data_len + 1 {
        return Err(Error::Protocol("mieru udp: truncated"));
    }
    if data[3 + data_len] != 0xff {
        return Err(Error::Protocol("mieru udp: missing end marker"));
    }

    Ok(data[3..3 + data_len].to_vec())
}

pub(crate) fn decode_inbound_udp_packet(data: &[u8]) -> Result<MieruInboundUdpPacket, Error> {
    let packet = unwrap_udp_associate(data)?;
    parse_socks5_udp_packet(&packet)
}

pub(crate) fn encode_udp_response(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let packet = build_socks5_udp_packet(target, port, payload)?;
    Ok(wrap_udp_associate(&packet))
}

pub(crate) fn decode_udp_flow_packet(data: &[u8]) -> Result<MieruInboundUdpPacket, Error> {
    decode_inbound_udp_packet(data)
}

pub(crate) fn encode_udp_flow_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_udp_response(target, port, payload)
}

/// Codec state for Mieru UDP flow datagrams.
///
/// Mieru UDP flow framing is stateless at this layer; stream encryption state is
/// owned by `MieruOutbound`.
#[derive(Debug, Default, Clone, Copy)]
pub struct MieruUdpFlowCodec;

impl MieruUdpFlowCodec {
    pub fn encode_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    pub fn decode_packet(&self, data: &[u8]) -> Result<MieruInboundUdpPacket, Error> {
        decode_udp_flow_packet(data)
    }

    #[cfg(feature = "crypto")]
    pub async fn write_response_tokio<W>(
        &self,
        writer: &mut W,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let frame = self.encode_packet(target, port, payload)?;
        let len = frame.len();
        tokio::io::AsyncWriteExt::write_all(writer, &frame)
            .await
            .map_err(|_| Error::Io("failed to write Mieru UDP response"))?;
        tokio::io::AsyncWriteExt::flush(writer)
            .await
            .map_err(|_| Error::Io("failed to flush Mieru UDP response"))?;
        Ok(len)
    }
}

pub(crate) fn udp_flow_codec() -> impl DatagramCodec<Address, Error = Error> {
    MieruUdpFlowCodec
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MieruUdpFlowResume {
    username: String,
    password: String,
    relay_chain: bool,
}

impl MieruUdpFlowResume {
    pub fn new(username: &str, password: &str, relay_chain: bool) -> Self {
        Self {
            username: username.to_owned(),
            password: password.to_owned(),
            relay_chain,
        }
    }

    pub(crate) fn username(&self) -> &str {
        &self.username
    }

    pub(crate) fn password(&self) -> &str {
        &self.password
    }

    pub fn flow_requires_relay_upstream(&self) -> bool {
        self.relay_chain
    }

    fn leaf_cache_key(&self, server: &str, port: u16) -> MieruUdpLeafKey {
        self.peer_config().leaf_cache_key(server, port)
    }

    fn flow_key(&self, server: &str, port: u16) -> MieruUdpFlowKey {
        if self.relay_chain {
            MieruUdpFlowKey::Relay
        } else {
            MieruUdpFlowKey::Leaf(self.leaf_cache_key(server, port))
        }
    }

    fn cache_key(&self, server: &str, port: u16, session_id: u64) -> MieruUdpCacheKey {
        MieruUdpCacheKey::from_flow_key(self.flow_key(server, port), session_id)
    }

    pub fn flow_cache_key(&self, server: &str, port: u16, session_id: u64) -> String {
        if self.relay_chain {
            return alloc::format!("relay|session:{session_id}");
        }
        let peer = self.peer_config();
        alloc::format!(
            "leaf|{server}:{port}|username:{}|password:{}",
            peer.username,
            peer.password
        )
    }

    pub fn connector_flow(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> MieruUdpConnectorFlow {
        MieruUdpConnectorFlow {
            cache_key: self.flow_cache_key(server, port, session_id),
            requires_relay_upstream: self.flow_requires_relay_upstream(),
        }
    }

    pub fn codec(&self) -> impl DatagramCodec<Address, Error = Error> {
        udp_flow_codec()
    }

    fn peer_config(&self) -> MieruUdpPeerConfig<'_> {
        MieruUdpPeerConfig {
            username: &self.username,
            password: &self.password,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MieruUdpConnectorFlow {
    cache_key: alloc::string::String,
    requires_relay_upstream: bool,
}

impl MieruUdpConnectorFlow {
    pub fn into_parts(self) -> (alloc::string::String, bool) {
        (self.cache_key, self.requires_relay_upstream)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MieruUdpFlowConfig<'a> {
    username: &'a str,
    password: &'a str,
}

impl<'a> MieruUdpFlowConfig<'a> {
    pub fn new(username: &'a str, password: &'a str) -> Self {
        Self { username, password }
    }

    pub fn flow_resume(&self, relay_chain: bool) -> MieruUdpFlowResume {
        MieruUdpFlowResume::new(self.username, self.password, relay_chain)
    }
}

pub fn udp_flow_resume_from_config(
    username: &str,
    password: &str,
    relay_chain: bool,
) -> MieruUdpFlowResume {
    MieruUdpFlowConfig::new(username, password).flow_resume(relay_chain)
}

pub fn connector_flow_from_resume(
    resume: &MieruUdpFlowResume,
    server: &str,
    port: u16,
    session_id: u64,
) -> MieruUdpConnectorFlow {
    resume.connector_flow(server, port, session_id)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum MieruUdpFlowKey {
    Leaf(MieruUdpLeafKey),
    Relay,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum MieruUdpCacheKey {
    Leaf(MieruUdpLeafKey),
    Relay { session_id: u64 },
}

impl MieruUdpCacheKey {
    fn from_flow_key(flow_key: MieruUdpFlowKey, session_id: u64) -> Self {
        match flow_key {
            MieruUdpFlowKey::Leaf(leaf_key) => Self::Leaf(leaf_key),
            MieruUdpFlowKey::Relay => Self::Relay { session_id },
        }
    }
}

pub struct MieruUdpFlowStore<T> {
    entries: alloc::collections::BTreeMap<MieruUdpCacheKey, T>,
}

impl<T> Default for MieruUdpFlowStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> MieruUdpFlowStore<T> {
    pub fn new() -> Self {
        Self {
            entries: alloc::collections::BTreeMap::new(),
        }
    }

    pub fn get(
        &self,
        resume: &MieruUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Option<&T> {
        let key = resume.cache_key(server, port, session_id);
        self.entries.get(&key)
    }

    pub fn insert(
        &mut self,
        resume: &MieruUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
        value: T,
    ) -> Option<T> {
        let key = resume.cache_key(server, port, session_id);
        self.entries.insert(key, value)
    }
}

#[cfg(feature = "crypto")]
#[derive(Default)]
pub struct MieruUdpFlowSessions {
    entries: MieruUdpFlowStore<crate::outbound::MieruUdpFlowConnection>,
}

#[cfg(feature = "crypto")]
impl MieruUdpFlowSessions {
    pub fn new() -> Self {
        Self {
            entries: MieruUdpFlowStore::new(),
        }
    }

    pub fn get(
        &self,
        resume: &MieruUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Option<&crate::outbound::MieruUdpFlowConnection> {
        self.entries.get(resume, server, port, session_id)
    }

    pub fn insert(
        &mut self,
        resume: &MieruUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
        connection: crate::outbound::MieruUdpFlowConnection,
    ) -> Option<crate::outbound::MieruUdpFlowConnection> {
        self.entries
            .insert(resume, server, port, session_id, connection)
    }
}

#[derive(Debug, Clone, Copy)]
struct MieruUdpPeerConfig<'a> {
    username: &'a str,
    password: &'a str,
}

impl<'a> MieruUdpPeerConfig<'a> {
    fn leaf_cache_key(&self, server: &str, port: u16) -> MieruUdpLeafKey {
        MieruUdpLeafKey {
            server: server.to_owned(),
            port,
            username: self.username.to_owned(),
            password: self.password.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct MieruUdpLeafKey {
    server: String,
    port: u16,
    username: String,
    password: String,
}

impl DatagramCodec<Address> for MieruUdpFlowCodec {
    type Error = Error;

    fn encode(&self, target: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, Self::Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    fn decode(&self, data: &[u8]) -> Option<(Address, u16, Vec<u8>)> {
        let decoded = decode_udp_flow_packet(data).ok()?;
        Some(decoded.into_parts())
    }
}

fn parse_socks5_udp_packet(packet: &[u8]) -> Result<MieruInboundUdpPacket, Error> {
    if packet.len() < 4 {
        return Err(Error::Protocol("mieru udp socks5 packet is too short"));
    }
    if packet[0] != 0 || packet[1] != 0 {
        return Err(Error::Protocol(
            "mieru udp socks5 packet has invalid reserved bytes",
        ));
    }
    if packet[2] != 0 {
        return Err(Error::Unsupported(
            "mieru udp socks5 fragmentation is not supported",
        ));
    }

    let mut offset = 4;
    let target = match packet[3] {
        ATYP_IPV4 => {
            if packet.len() < offset + 4 + 2 {
                return Err(Error::Protocol("mieru udp socks5 ipv4 packet is truncated"));
            }
            let mut bytes = [0_u8; 4];
            bytes.copy_from_slice(&packet[offset..offset + 4]);
            offset += 4;
            Address::Ipv4(bytes)
        }
        ATYP_IPV6 => {
            if packet.len() < offset + 16 + 2 {
                return Err(Error::Protocol("mieru udp socks5 ipv6 packet is truncated"));
            }
            let mut bytes = [0_u8; 16];
            bytes.copy_from_slice(&packet[offset..offset + 16]);
            offset += 16;
            Address::Ipv6(bytes)
        }
        ATYP_DOMAIN => {
            if packet.len() < offset + 1 {
                return Err(Error::Protocol(
                    "mieru udp socks5 domain packet is truncated",
                ));
            }
            let len = packet[offset] as usize;
            offset += 1;
            if len == 0 || packet.len() < offset + len + 2 {
                return Err(Error::Protocol(
                    "mieru udp socks5 domain packet is truncated",
                ));
            }
            let domain = String::from_utf8(packet[offset..offset + len].to_vec())
                .map_err(|_| Error::Protocol("mieru udp socks5 domain is not valid UTF-8"))?;
            offset += len;
            Address::Domain(domain)
        }
        _ => {
            return Err(Error::Unsupported(
                "mieru udp socks5 address type is not supported",
            ))
        }
    };

    let port = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
    offset += 2;

    Ok(MieruInboundUdpPacket::new(
        target,
        port,
        packet[offset..].to_vec(),
    ))
}

fn build_socks5_udp_packet(address: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
    let mut packet = vec![0_u8, 0_u8, 0_u8];
    write_socks5_address(&mut packet, address)?;
    packet.extend_from_slice(&port.to_be_bytes());
    packet.extend_from_slice(payload);
    Ok(packet)
}

fn write_socks5_address(packet: &mut Vec<u8>, address: &Address) -> Result<(), Error> {
    match address {
        Address::Ipv4(bytes) => {
            packet.push(ATYP_IPV4);
            packet.extend_from_slice(bytes);
        }
        Address::Ipv6(bytes) => {
            packet.push(ATYP_IPV6);
            packet.extend_from_slice(bytes);
        }
        Address::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.is_empty() {
                return Err(Error::Protocol("mieru udp socks5 domain must not be empty"));
            }
            if bytes.len() > u8::MAX as usize {
                return Err(Error::Unsupported("mieru udp socks5 domain is too long"));
            }
            packet.push(ATYP_DOMAIN);
            packet.push(bytes.len() as u8);
            packet.extend_from_slice(bytes);
        }
    }
    Ok(())
}
