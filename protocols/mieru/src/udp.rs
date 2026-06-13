// Mieru UDP associate encapsulation — udp.rs
//
// SOCKS5 UDP ASSOCIATE over mieru wraps datagrams with markers:
//   [0x00] [len: 2 bytes BE] [data: len bytes] [0xff]
//
// This preserves datagram boundaries when transmitted over TCP streams.

use alloc::vec::Vec;

use zero_core::Error;

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
