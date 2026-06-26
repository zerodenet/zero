// Mieru UDP associate encapsulation — udp.rs
//
// SOCKS5 UDP ASSOCIATE over mieru wraps datagrams with markers:
//   [0x00] [len: 2 bytes BE] [data: len bytes] [0xff]
//
// This preserves datagram boundaries when transmitted over TCP streams.

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::DatagramCodec;

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
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MieruInboundUdpPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

/// Wrap a raw UDP datagram for transmission through mieru TCP/UDP proxy.
///
/// Format: 0x00 || data_length(u16 BE) || data || 0xff
pub fn wrap_udp_associate(data: &[u8]) -> Vec<u8> {
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
pub fn unwrap_udp_associate(data: &[u8]) -> Result<Vec<u8>, Error> {
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

pub fn decode_inbound_udp_packet(data: &[u8]) -> Result<MieruInboundUdpPacket, Error> {
    let packet = unwrap_udp_associate(data)?;
    parse_socks5_udp_packet(&packet)
}

pub fn encode_udp_response(target: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
    let packet = build_socks5_udp_packet(target, port, payload)?;
    Ok(wrap_udp_associate(&packet))
}

pub fn decode_udp_flow_packet(data: &[u8]) -> Result<MieruInboundUdpPacket, Error> {
    decode_inbound_udp_packet(data)
}

pub fn encode_udp_flow_packet(
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
}

pub fn udp_flow_codec() -> impl DatagramCodec<Address, Error = Error> {
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

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn password(&self) -> &str {
        &self.password
    }

    pub fn relay_chain(&self) -> bool {
        self.relay_chain
    }

    pub fn flow_requires_relay_upstream(&self) -> bool {
        self.relay_chain
    }

    pub fn leaf_cache_key(&self, server: &str, port: u16) -> MieruUdpLeafKey {
        self.peer_config().leaf_cache_key(server, port)
    }

    pub fn flow_key(&self, server: &str, port: u16) -> MieruUdpFlowKey {
        if self.relay_chain {
            MieruUdpFlowKey::Relay
        } else {
            MieruUdpFlowKey::Leaf(self.leaf_cache_key(server, port))
        }
    }

    pub fn cache_key(&self, server: &str, port: u16, session_id: u64) -> MieruUdpCacheKey {
        MieruUdpCacheKey::from_flow_key(self.flow_key(server, port), session_id)
    }

    pub fn codec(&self) -> impl DatagramCodec<Address, Error = Error> {
        udp_flow_codec()
    }

    pub fn peer_config(&self) -> MieruUdpPeerConfig<'_> {
        MieruUdpPeerConfig {
            username: &self.username,
            password: &self.password,
            relay_chain: self.relay_chain,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MieruUdpFlowKey {
    Leaf(MieruUdpLeafKey),
    Relay,
}

impl MieruUdpFlowKey {
    pub fn is_relay(&self) -> bool {
        matches!(self, Self::Relay)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MieruUdpCacheKey {
    Leaf(MieruUdpLeafKey),
    Relay { session_id: u64 },
}

impl MieruUdpCacheKey {
    pub fn from_flow_key(flow_key: MieruUdpFlowKey, session_id: u64) -> Self {
        match flow_key {
            MieruUdpFlowKey::Leaf(leaf_key) => Self::Leaf(leaf_key),
            MieruUdpFlowKey::Relay => Self::Relay { session_id },
        }
    }

    pub fn relay(session_id: u64) -> Self {
        Self::Relay { session_id }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MieruUdpPeerConfig<'a> {
    username: &'a str,
    password: &'a str,
    relay_chain: bool,
}

impl<'a> MieruUdpPeerConfig<'a> {
    pub fn username(&self) -> &'a str {
        self.username
    }

    pub fn password(&self) -> &'a str {
        self.password
    }

    pub fn relay_chain(&self) -> bool {
        self.relay_chain
    }

    pub fn leaf_cache_key(&self, server: &str, port: u16) -> MieruUdpLeafKey {
        MieruUdpLeafKey {
            server: server.to_owned(),
            port,
            username: self.username.to_owned(),
            password: self.password.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MieruUdpLeafKey {
    server: String,
    port: u16,
    username: String,
    password: String,
}

impl MieruUdpLeafKey {
    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

impl DatagramCodec<Address> for MieruUdpFlowCodec {
    type Error = Error;

    fn encode(&self, target: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, Self::Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    fn decode(&self, data: &[u8]) -> Option<(Address, u16, Vec<u8>)> {
        let decoded = decode_udp_flow_packet(data).ok()?;
        Some((decoded.target, decoded.port, decoded.payload))
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

    Ok(MieruInboundUdpPacket {
        target,
        port,
        payload: packet[offset..].to_vec(),
    })
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
