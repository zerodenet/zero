// Hysteria2 UDP datagram — udp.rs

use alloc::string::String;
use alloc::vec::Vec;

use zero_core::{Address, Error};

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
