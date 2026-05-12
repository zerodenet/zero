// VLESS MUX (Connection Multiplexing) — mux.rs
//
// Encodes multiple TCP streams within a single VLESS connection.
//
// Frame format (Xray-compatible subset):
//   0               1               2               3
//   0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7
//  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//  |          session_id (u16 BE)  |           length (u16 BE)     |
//  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//  |                       payload (length bytes)                  |
//  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//
// Control frames (session_id == 0):
//   — New stream request (client → server):
//       payload: [u16:port] [u8:atyp] [address…]
//   — New stream response (server → client):
//       payload: [u16:assigned_id] [u8:status(0=ok,1=fail)]
//
// Data frames (session_id > 0):
//   — payload: raw stream bytes (fits within single TLS record)

use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

use crate::shared::{read_exact, write_address, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6};

pub const MUX_FRAME_HEADER_LEN: usize = 4;
pub const MUX_MAX_PAYLOAD: usize = 16384; // keep inside one TLS record
pub const MUX_STREAM_NEW: u16 = 0;
pub const MUX_STATUS_OK: u8 = 0x00;
pub const MUX_STATUS_FAIL: u8 = 0x01;

/// Parsed MUX frame.
#[derive(Debug, Clone)]
pub struct MuxFrame {
    pub stream_id: u16,
    pub payload: Vec<u8>,
}

/// ——— frame encode / decode ———

pub fn encode_frame(stream_id: u16, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&stream_id.to_be_bytes());
    buf.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

pub fn encode_new_stream(port: u16, address: &Address) -> Result<Vec<u8>, Error> {
    let mut payload = Vec::with_capacity(24);
    payload.extend_from_slice(&port.to_be_bytes());
    write_address(&mut payload, address)?;
    Ok(encode_frame(MUX_STREAM_NEW, &payload))
}

pub fn encode_new_stream_response(assigned_id: u16, status: u8) -> Vec<u8> {
    let mut payload = Vec::with_capacity(3);
    payload.extend_from_slice(&assigned_id.to_be_bytes());
    payload.push(status);
    encode_frame(MUX_STREAM_NEW, &payload)
}

pub fn parse_new_stream_payload(payload: &[u8]) -> Result<(u16, Address), Error> {
    if payload.len() < 3 {
        return Err(Error::Protocol("MUX new stream payload too short"));
    }
    let port = u16::from_be_bytes([payload[0], payload[1]]);
    if port == 0 {
        return Err(Error::Protocol("MUX target port must not be 0"));
    }
    let atyp = payload[2];
    let target = parse_address_from_bytes(atyp, &payload[3..])?;
    Ok((port, target))
}

pub fn parse_new_stream_response(payload: &[u8]) -> Result<(u16, u8), Error> {
    if payload.len() < 3 {
        return Err(Error::Protocol("MUX new stream response too short"));
    }
    Ok((u16::from_be_bytes([payload[0], payload[1]]), payload[2]))
}

fn parse_address_from_bytes(atyp: u8, data: &[u8]) -> Result<Address, Error> {
    match atyp {
        ATYP_IPV4 => {
            if data.len() < 4 {
                return Err(Error::Protocol("MUX: truncated IPv4 address"));
            }
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&data[..4]);
            Ok(Address::Ipv4(bytes))
        }
        ATYP_IPV6 => {
            if data.len() < 16 {
                return Err(Error::Protocol("MUX: truncated IPv6 address"));
            }
            let mut bytes = [0u8; 16];
            bytes.copy_from_slice(&data[..16]);
            Ok(Address::Ipv6(bytes))
        }
        ATYP_DOMAIN => {
            if data.is_empty() {
                return Err(Error::Protocol("MUX: truncated domain address"));
            }
            let len = data[0] as usize;
            if len == 0 || data.len() < 1 + len {
                return Err(Error::Protocol("MUX: truncated domain address"));
            }
            let domain = alloc::string::String::from_utf8(data[1..1 + len].to_vec())
                .map_err(|_| Error::Protocol("MUX domain not valid UTF-8"))?;
            Ok(Address::Domain(domain))
        }
        _ => Err(Error::Unsupported("MUX address type not supported")),
    }
}

/// Read a complete MUX frame from the stream.
pub async fn read_mux_frame<S>(stream: &mut S) -> Result<MuxFrame, Error>
where
    S: AsyncSocket,
{
    let mut header = [0u8; 4];
    read_exact(stream, &mut header).await?;
    let stream_id = u16::from_be_bytes([header[0], header[1]]);
    let length = u16::from_be_bytes([header[2], header[3]]) as usize;

    if length as usize > MUX_MAX_PAYLOAD {
        return Err(Error::Protocol("MUX frame payload too large"));
    }

    let mut payload = alloc::vec![0u8; length];
    if length > 0 {
        read_exact(stream, &mut payload).await?;
    }

    Ok(MuxFrame { stream_id, payload })
}

/// ——— mux client ———————————————————————————

/// State for one MUX stream on the client side.
#[derive(Debug)]
pub struct MuxClientStream {
    pub id: u16,
}

/// Minimal MUX client — manages stream allocation and frame I/O.
pub struct MuxClient {
    next_id: u16,
}

impl MuxClient {
    pub fn new() -> Self {
        Self { next_id: 1 }
    }

    /// Allocate next available stream ID.
    pub fn alloc_id(&mut self) -> u16 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        id
    }

    /// Send a new-stream request and return the server-assigned ID.
    pub async fn open_stream<S>(
        &self,
        stream: &mut S,
        port: u16,
        address: &Address,
    ) -> Result<(u16, MuxClientStream), Error>
    where
        S: AsyncSocket,
    {
        let req = encode_new_stream(port, address)?;
        stream
            .write_all(&req)
            .await
            .map_err(|_| Error::Io("failed to write MUX new-stream request"))?;

        let frame = read_mux_frame(stream).await?;
        if frame.stream_id != MUX_STREAM_NEW {
            return Err(Error::Protocol("expected MUX new-stream response"));
        }
        let (assigned_id, status) = parse_new_stream_response(&frame.payload)?;
        if status != MUX_STATUS_OK {
            return Err(Error::Protocol("MUX server rejected new stream"));
        }

        Ok((assigned_id, MuxClientStream { id: assigned_id }))
    }

    /// Write data to a stream.
    pub async fn write_data<S>(&self, stream: &mut S, sid: u16, data: &[u8]) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let frame = encode_frame(sid, data);
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX data frame"))
    }

    /// Read next incoming frame from server.
    pub async fn recv<S>(&self, stream: &mut S) -> Result<MuxFrame, Error>
    where
        S: AsyncSocket,
    {
        read_mux_frame(stream).await
    }
}

/// ——— mux server ———————————————————————————

/// MUX server-side handler — reads frames and dispatches.
pub struct MuxServer;

impl MuxServer {
    pub fn new() -> Self {
        Self
    }

    /// Accept a new stream request, allocate an ID, and send response.
    pub async fn accept_new_stream<S>(
        &self,
        stream: &mut S,
        alloc_id: u16,
    ) -> Result<(u16, u16, Address), Error>
    where
        S: AsyncSocket,
    {
        let frame = read_mux_frame(stream).await?;
        if frame.stream_id != MUX_STREAM_NEW {
            return Err(Error::Protocol("expected MUX new-stream request"));
        }

        let (port, target) = parse_new_stream_payload(&frame.payload)?;

        let resp = encode_new_stream_response(alloc_id, MUX_STATUS_OK);
        stream
            .write_all(&resp)
            .await
            .map_err(|_| Error::Io("failed to write MUX new-stream response"))?;

        Ok((alloc_id, port, target))
    }

    /// Read next frame.
    pub async fn recv<S>(&self, stream: &mut S) -> Result<MuxFrame, Error>
    where
        S: AsyncSocket,
    {
        read_mux_frame(stream).await
    }

    /// Write data to a stream.
    pub async fn write_data<S>(&self, stream: &mut S, sid: u16, data: &[u8]) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let frame = encode_frame(sid, data);
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX data frame"))
    }
}
