// Hysteria2 protocol constants and helpers — shared.rs

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

pub const HYSTERIA2_VERSION: u8 = 0x02;

pub const AUTH_OK: u8 = 0x01;
pub const AUTH_ERR: u8 = 0x00;

pub const STREAM_TYPE_TCP: u8 = 0x00;
pub const STREAM_TYPE_UDP: u8 = 0x01;

pub const ADDR_TYPE_IPV4: u8 = 0x01;
pub const ADDR_TYPE_DOMAIN: u8 = 0x02;
pub const ADDR_TYPE_IPV6: u8 = 0x03;

/// Build an authentication frame to send to the server.
/// Format: [version:1][auth_len:2][auth_payload:auth_len]
pub fn build_auth_frame(hmac: &[u8; 32]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(3 + 32);
    frame.push(HYSTERIA2_VERSION);
    frame.extend_from_slice(&32u16.to_be_bytes());
    frame.extend_from_slice(hmac);
    frame
}

/// Parse an authentication response from the server.
/// Format: [ok:1][version:1] for success, [err:1][msg_len:2][msg] for failure.
pub fn parse_auth_response(data: &[u8]) -> Result<(), Error> {
    if data.is_empty() {
        return Err(Error::Protocol("hysteria2: empty auth response"));
    }
    match data[0] {
        AUTH_OK => Ok(()),
        AUTH_ERR => {
            if data.len() < 3 {
                return Err(Error::Protocol("hysteria2: truncated auth error"));
            }
            let msg_len = u16::from_be_bytes([data[1], data[2]]) as usize;
            let msg = if data.len() >= 3 + msg_len {
                String::from_utf8_lossy(&data[3..3 + msg_len]).into_owned()
            } else {
                String::from("unknown error")
            };
            Err(Error::Protocol("hysteria2 auth rejected"))
        }
        _ => Err(Error::Protocol("hysteria2: unknown auth response type")),
    }
}

/// Parse a read auth frame from the client.
/// Returns the HMAC bytes.
pub fn parse_auth_frame(data: &[u8]) -> Result<[u8; 32], Error> {
    if data.len() < 3 {
        return Err(Error::Protocol("hysteria2: truncated auth frame"));
    }
    let version = data[0];
    if version != HYSTERIA2_VERSION {
        return Err(Error::Protocol("hysteria2: unsupported version"));
    }
    let auth_len = u16::from_be_bytes([data[1], data[2]]) as usize;
    if auth_len != 32 {
        return Err(Error::Protocol("hysteria2: invalid auth length"));
    }
    if data.len() < 3 + 32 {
        return Err(Error::Protocol("hysteria2: truncated auth payload"));
    }
    let mut hmac = [0u8; 32];
    hmac.copy_from_slice(&data[3..35]);
    Ok(hmac)
}

/// Build a TCP stream connect header.
/// Format: [type:1][addr_len:2][addr:var][port:2]
pub fn build_tcp_connect_header(address: &Address, port: u16) -> Result<Vec<u8>, Error> {
    let addr_bytes = encode_address(address)?;
    let mut header = Vec::with_capacity(3 + addr_bytes.len() + 2);
    header.push(STREAM_TYPE_TCP);
    header.extend_from_slice(&(addr_bytes.len() as u16).to_be_bytes());
    header.extend_from_slice(&addr_bytes);
    header.extend_from_slice(&port.to_be_bytes());
    Ok(header)
}

/// Parse a TCP stream connect header.
/// Returns the target address and port.
pub fn parse_tcp_connect_header(data: &[u8]) -> Result<(Address, u16), Error> {
    if data.len() < 4 {
        return Err(Error::Protocol("hysteria2: truncated connect header"));
    }
    let stream_type = data[0];
    if stream_type != STREAM_TYPE_TCP {
        return Err(Error::Protocol("hysteria2: expected TCP stream"));
    }
    let addr_len = u16::from_be_bytes([data[1], data[2]]) as usize;
    if data.len() < 3 + addr_len + 2 {
        return Err(Error::Protocol("hysteria2: truncated address in connect header"));
    }
    let addr = decode_address(&data[3..3 + addr_len])?;
    let port = u16::from_be_bytes([data[3 + addr_len], data[3 + addr_len + 1]]);
    Ok((addr, port))
}

/// Build an auth error response.
pub fn build_auth_error(msg: &str) -> Vec<u8> {
    let msg_bytes = msg.as_bytes();
    let mut resp = Vec::with_capacity(3 + msg_bytes.len());
    resp.push(AUTH_ERR);
    resp.extend_from_slice(&(msg_bytes.len() as u16).to_be_bytes());
    resp.extend_from_slice(msg_bytes);
    resp
}

/// Build an auth success response.
pub fn build_auth_ok() -> Vec<u8> {
    vec![AUTH_OK, HYSTERIA2_VERSION]
}

/// Build a TCP connect success response.
pub fn build_connect_ok() -> Vec<u8> {
    vec![0x01]
}

/// Build a TCP connect error response.
pub fn build_connect_error(msg: &str) -> Vec<u8> {
    let msg_bytes = msg.as_bytes();
    let mut resp = Vec::with_capacity(3 + msg_bytes.len());
    resp.push(0x00);
    resp.extend_from_slice(&(msg_bytes.len() as u16).to_be_bytes());
    resp.extend_from_slice(msg_bytes);
    resp
}

// — address encoding helpers —

pub(crate) fn encode_address(addr: &Address) -> Result<Vec<u8>, Error> {
    match addr {
        Address::Ipv4(bytes) => {
            let mut buf = Vec::with_capacity(5);
            buf.push(ADDR_TYPE_IPV4);
            buf.extend_from_slice(bytes);
            Ok(buf)
        }
        Address::Ipv6(bytes) => {
            let mut buf = Vec::with_capacity(17);
            buf.push(ADDR_TYPE_IPV6);
            buf.extend_from_slice(bytes);
            Ok(buf)
        }
        Address::Domain(domain) => {
            let b = domain.as_bytes();
            if b.is_empty() || b.len() > u8::MAX as usize {
                return Err(Error::Protocol("hysteria2: invalid domain length"));
            }
            let mut buf = Vec::with_capacity(2 + b.len());
            buf.push(ADDR_TYPE_DOMAIN);
            buf.push(b.len() as u8);
            buf.extend_from_slice(b);
            Ok(buf)
        }
    }
}

fn decode_address(data: &[u8]) -> Result<Address, Error> {
    if data.is_empty() {
        return Err(Error::Protocol("hysteria2: empty address data"));
    }
    match data[0] {
        ADDR_TYPE_IPV4 => {
            if data.len() < 5 {
                return Err(Error::Protocol("hysteria2: truncated IPv4"));
            }
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[1..5]);
            Ok(Address::Ipv4(bytes))
        }
        ADDR_TYPE_IPV6 => {
            if data.len() < 17 {
                return Err(Error::Protocol("hysteria2: truncated IPv6"));
            }
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&data[1..17]);
            Ok(Address::Ipv6(bytes))
        }
        ADDR_TYPE_DOMAIN => {
            let len = data[1] as usize;
            if data.len() < 2 + len {
                return Err(Error::Protocol("hysteria2: truncated domain"));
            }
            let domain =
                String::from_utf8(data[2..2 + len].to_vec()).map_err(|_| Error::Protocol("hysteria2: invalid domain UTF-8"))?;
            Ok(Address::Domain(domain))
        }
        _ => Err(Error::Unsupported("hysteria2: unknown address type")),
    }
}

/// Read exact number of bytes from stream.
pub async fn read_exact<S: AsyncSocket>(stream: &mut S, buf: &mut [u8]) -> Result<(), Error> {
    let mut offset = 0;
    while offset < buf.len() {
        let n = stream
            .read(&mut buf[offset..])
            .await
            .map_err(|_| Error::Io("hysteria2: read failed"))?;
        if n == 0 {
            return Err(Error::Io("hysteria2: unexpected EOF"));
        }
        offset += n;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_frame_roundtrip() {
        let hmac = [0xAAu8; 32];
        let frame = build_auth_frame(&hmac);
        assert_eq!(frame[0], 0x02);
        let parsed = parse_auth_frame(&frame).unwrap();
        assert_eq!(parsed, hmac);
    }

    #[test]
    fn test_auth_response_ok() {
        let resp = build_auth_ok();
        assert!(parse_auth_response(&resp).is_ok());
    }

    #[test]
    fn test_auth_response_err() {
        let resp = build_auth_error("bad password");
        assert!(parse_auth_response(&resp).is_err());
    }

    #[test]
    fn test_tcp_connect_header_roundtrip() {
        let addr = Address::Domain("example.com".into());
        let header = build_tcp_connect_header(&addr, 443).unwrap();
        let (parsed_addr, parsed_port) = parse_tcp_connect_header(&header).unwrap();
        assert_eq!(parsed_addr, addr);
        assert_eq!(parsed_port, 443);
    }

    #[test]
    fn test_tcp_connect_ipv4() {
        let addr = Address::Ipv4([127, 0, 0, 1]);
        let header = build_tcp_connect_header(&addr, 80).unwrap();
        let (parsed_addr, parsed_port) = parse_tcp_connect_header(&header).unwrap();
        assert_eq!(parsed_addr, addr);
        assert_eq!(parsed_port, 80);
    }

    #[test]
    fn test_connect_response_ok() {
        let resp = build_connect_ok();
        assert_eq!(resp[0], 0x01);
    }

    #[test]
    fn test_connect_response_err() {
        let resp = build_connect_error("connection refused");
        assert_eq!(resp[0], 0x00);
        let msg_len = u16::from_be_bytes([resp[1], resp[2]]) as usize;
        assert_eq!(&resp[3..3 + msg_len], b"connection refused");
    }
}
