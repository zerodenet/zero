use alloc::vec;
use alloc::vec::Vec;

use zero_core::{Address, Error, InboundUdpDispatch, ProtocolType};

use crate::shared::{write_address, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Socks5UdpPacket {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl Socks5UdpPacket {
    pub(crate) fn new(target: Address, port: u16, payload: Vec<u8>) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub(crate) fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5InboundUdpRequest {
    target: Address,
    port: u16,
    payload: Vec<u8>,
    frame_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Socks5InboundUdpDispatchParts {
    target: Address,
    port: u16,
    payload: Vec<u8>,
    client_session_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5InboundUdpDispatchView {
    parts: Socks5InboundUdpDispatchParts,
    protocol_overhead_len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Socks5InboundUdpProtocolOverhead {
    bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Socks5InboundUdpDispatchAction {
    LocalDns { domain: alloc::string::String },
    Dispatch(Socks5InboundUdpDispatchView),
}

impl Socks5InboundUdpDispatchView {
    pub fn protocol(&self) -> ProtocolType {
        self.parts.protocol()
    }

    pub fn into_pipe_parts(self) -> (Address, u16, Vec<u8>, Option<u64>) {
        self.parts.into_parts()
    }

    pub fn into_inbound_dispatch(self) -> InboundUdpDispatch {
        self.parts.into_inbound_dispatch()
    }

    pub fn pipe_parts(&self) -> (&Address, u16, &[u8], Option<u64>) {
        self.parts.pipe_parts()
    }

    pub fn protocol_overhead(&self) -> Socks5InboundUdpProtocolOverhead {
        Socks5InboundUdpProtocolOverhead {
            bytes: self.protocol_overhead_len,
        }
    }

    pub fn record_protocol_overhead<F>(&self, session_id: u64, record: F)
    where
        F: FnOnce(u64, u64),
    {
        record(session_id, self.protocol_overhead_len as u64);
    }
}

impl Socks5InboundUdpProtocolOverhead {
    pub fn record<F>(self, session_id: u64, record: F)
    where
        F: FnOnce(u64, u64),
    {
        record(session_id, self.bytes as u64);
    }
}

impl Socks5InboundUdpDispatchParts {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Socks5
    }

    pub fn pipe_parts(&self) -> (&Address, u16, &[u8], Option<u64>) {
        (
            &self.target,
            self.port,
            &self.payload,
            self.client_session_id,
        )
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>, Option<u64>) {
        (self.target, self.port, self.payload, self.client_session_id)
    }

    pub fn into_inbound_dispatch(self) -> InboundUdpDispatch {
        let (target, port, payload, client_session_id) = self.into_parts();
        InboundUdpDispatch::new(
            ProtocolType::Socks5,
            target,
            port,
            payload,
            client_session_id,
        )
    }
}

impl Socks5InboundUdpRequest {
    pub(crate) fn from_packet(packet: Socks5UdpPacket, frame_len: usize) -> Self {
        let (target, port, payload) = packet.into_parts();
        Self {
            target,
            port,
            payload,
            frame_len,
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

    pub fn is_dns_domain_request(&self) -> bool {
        matches!(&self.target, Address::Domain(_)) && self.port == 53
    }

    pub fn dns_domain_request(&self) -> Option<&str> {
        match (&self.target, self.port) {
            (Address::Domain(domain), 53) => Some(domain),
            _ => None,
        }
    }

    pub fn protocol_overhead_len(&self) -> usize {
        self.frame_len.saturating_sub(self.payload.len())
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }

    pub(crate) fn into_dispatch_parts(self) -> Socks5InboundUdpDispatchParts {
        let (target, port, payload) = self.into_parts();
        Socks5InboundUdpDispatchParts {
            target,
            port,
            payload,
            client_session_id: None,
        }
    }

    pub(crate) fn into_dispatch_action(self) -> Socks5InboundUdpDispatchAction {
        if let (Address::Domain(domain), 53) = (&self.target, self.port) {
            return Socks5InboundUdpDispatchAction::LocalDns {
                domain: domain.clone(),
            };
        }

        let protocol_overhead_len = self.protocol_overhead_len();
        Socks5InboundUdpDispatchAction::Dispatch(Socks5InboundUdpDispatchView {
            parts: self.into_dispatch_parts(),
            protocol_overhead_len,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5InboundUdpResponse {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl Socks5InboundUdpResponse {
    pub(crate) fn from_packet(packet: Socks5UdpPacket) -> Self {
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

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}

pub(crate) fn parse_udp_packet(packet: &[u8]) -> Result<Socks5UdpPacket, Error> {
    if packet.len() < 4 {
        return Err(Error::Protocol("SOCKS5 UDP packet is too short"));
    }

    if packet[0] != 0 || packet[1] != 0 {
        return Err(Error::Protocol(
            "SOCKS5 UDP packet has invalid reserved bytes",
        ));
    }

    if packet[2] != 0 {
        return Err(Error::Unsupported(
            "SOCKS5 UDP fragmentation is not supported",
        ));
    }

    let atyp = packet[3];
    let mut offset = 4;

    let target = match atyp {
        ATYP_IPV4 => {
            if packet.len() < offset + 4 + 2 {
                return Err(Error::Protocol("SOCKS5 UDP IPv4 packet is truncated"));
            }
            let mut bytes = [0_u8; 4];
            bytes.copy_from_slice(&packet[offset..offset + 4]);
            offset += 4;
            Address::Ipv4(bytes)
        }
        ATYP_IPV6 => {
            if packet.len() < offset + 16 + 2 {
                return Err(Error::Protocol("SOCKS5 UDP IPv6 packet is truncated"));
            }
            let mut bytes = [0_u8; 16];
            bytes.copy_from_slice(&packet[offset..offset + 16]);
            offset += 16;
            Address::Ipv6(bytes)
        }
        ATYP_DOMAIN => {
            if packet.len() < offset + 1 {
                return Err(Error::Protocol("SOCKS5 UDP domain packet is truncated"));
            }
            let len = packet[offset] as usize;
            offset += 1;
            if len == 0 || packet.len() < offset + len + 2 {
                return Err(Error::Protocol("SOCKS5 UDP domain packet is truncated"));
            }
            let domain = alloc::string::String::from_utf8(packet[offset..offset + len].to_vec())
                .map_err(|_| Error::Protocol("SOCKS5 UDP domain is not valid UTF-8"))?;
            offset += len;
            Address::Domain(domain)
        }
        _ => {
            return Err(Error::Unsupported(
                "SOCKS5 UDP address type is not supported",
            ))
        }
    };

    let port = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
    offset += 2;

    Ok(Socks5UdpPacket::new(
        target,
        port,
        packet[offset..].to_vec(),
    ))
}

pub(crate) fn build_udp_packet(
    address: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let mut packet = vec![0_u8, 0_u8, 0_u8];
    write_address(&mut packet, address)?;
    packet.extend_from_slice(&port.to_be_bytes());
    packet.extend_from_slice(payload);
    Ok(packet)
}

pub(crate) fn decode_udp_associate_request(packet: &[u8]) -> Result<Socks5UdpPacket, Error> {
    parse_udp_packet(packet)
}

pub(crate) fn decode_udp_associate_response(packet: &[u8]) -> Result<Socks5UdpPacket, Error> {
    parse_udp_packet(packet)
}

pub(crate) fn encode_udp_associate_response(
    address: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    build_udp_packet(address, port, payload)
}

pub(crate) fn encode_udp_associate_response_to_client(
    upstream_address: &Address,
    upstream_port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_udp_associate_response(upstream_address, upstream_port, payload)
}
