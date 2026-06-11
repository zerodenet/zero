// VLESS MUX (Connection Multiplexing) — mux.rs
//
// Encodes multiple TCP/UDP streams within a single VLESS connection.
//
// Frame format (Xray Mux.Cool compatible):
//   0               1               2
//   0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7
//  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//  |              length (u16 BE)                      |
//  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//  |            session_id (u16 BE)                    |
//  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//  |   status (u8)    |   options (u8)                 |
//  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//  |               payload (variable)                  |
//  +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//
// length covers session_id(2) + status(1) + options(1) + payload
//
// Status codes:
//   0x01 StatusNew      — New connection request
//   0x02 StatusKeep     — Ongoing session data
//   0x03 StatusEnd      — Session termination
//   0x04 StatusKeepAlive — Keep-alive signal
//
// New Stream request (session_id=0, status=STATUS_NEW):
//   payload: [network:1][port:2][atyp:1][address…]
// New Stream response (session_id=0, status=STATUS_NEW):
//   payload: [assigned_id:2][status:1(0=ok,1=fail)]
//
// Data frames (status=STATUS_KEEP, options=OPTION_DATA):
//   TCP: [payload_bytes…]
//   UDP: [network:1][port:2][atyp:1][address…][payload_bytes…]

use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

use crate::shared::{read_exact, write_address, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6};

// ── Constants ──

pub const MUX_FRAME_HEADER_LEN: usize = 6;
pub const MUX_MAX_PAYLOAD: usize = 16384; // keep inside one TLS record

// Session ID 0 for control frames (new stream, keepalive)
pub const MUX_STREAM_NEW: u16 = 0;

// Status codes
pub const STATUS_NEW: u8 = 0x01;
pub const STATUS_KEEP: u8 = 0x02;
pub const STATUS_END: u8 = 0x03;
pub const STATUS_KEEP_ALIVE: u8 = 0x04;

// Option flags
pub const OPTION_DATA: u8 = 0x01;

// Network types
pub const NETWORK_TCP: u8 = 0x01;
pub const NETWORK_UDP: u8 = 0x02;

// Backward-compat aliases for network type constants
pub const MUX_NETWORK_TCP: u8 = NETWORK_TCP;
pub const MUX_NETWORK_UDP: u8 = NETWORK_UDP;

// Response status (for new stream response)
pub const MUX_STATUS_OK: u8 = 0x00;
pub const MUX_STATUS_FAIL: u8 = 0x01;

// ── Types ──

/// Parsed MUX frame.
#[derive(Debug, Clone)]
pub struct MuxFrame {
    pub session_id: u16,
    pub status: u8,
    pub options: u8,
    pub payload: Vec<u8>,
}

/// Target info for a new MUX stream.
#[derive(Debug, Clone)]
pub struct MuxTarget {
    pub network: u8,
    pub port: u16,
    pub address: Address,
}

// ── frame encode / decode ──

/// Encode a MUX frame: [length:2(BE)][session_id:2(BE)][status:1][options:1][payload…]
/// length covers session_id(2) + status(1) + options(1) + payload.
pub fn encode_frame(session_id: u16, status: u8, options: u8, payload: &[u8]) -> Vec<u8> {
    // length = 4 + payload.len() (session_id:2 + status:1 + options:1 + payload)
    let total_len = 4u16
        .checked_add(payload.len() as u16)
        .expect("MUX frame payload too large for u16 length");
    let mut buf = Vec::with_capacity(6 + payload.len());
    buf.extend_from_slice(&total_len.to_be_bytes());
    buf.extend_from_slice(&session_id.to_be_bytes());
    buf.push(status);
    buf.push(options);
    buf.extend_from_slice(payload);
    buf
}

/// Read a complete MUX frame from the stream.
pub async fn read_mux_frame<S>(stream: &mut S) -> Result<MuxFrame, Error>
where
    S: AsyncSocket,
{
    let mut header = [0u8; MUX_FRAME_HEADER_LEN];
    read_exact(stream, &mut header).await?;

    let total_len = u16::from_be_bytes([header[0], header[1]]) as usize;
    if total_len < 4 {
        return Err(Error::Protocol("MUX frame length too short (min 4)"));
    }
    let session_id = u16::from_be_bytes([header[2], header[3]]);
    let status = header[4];
    let options = header[5];

    let payload_len = total_len
        .checked_sub(4)
        .ok_or(Error::Protocol("MUX frame length underflow"))?;

    if payload_len > MUX_MAX_PAYLOAD {
        return Err(Error::Protocol("MUX frame payload too large"));
    }

    let mut payload = alloc::vec![0u8; payload_len];
    if payload_len > 0 {
        read_exact(stream, &mut payload).await?;
    }

    Ok(MuxFrame {
        session_id,
        status,
        options,
        payload,
    })
}

// ── New stream request/response ──

/// Build a new-stream request frame (session_id=0, status=STATUS_NEW).
/// payload: [network:1][port:2][atyp:1][address…]
pub fn encode_new_stream(network: u8, port: u16, address: &Address) -> Result<Vec<u8>, Error> {
    let mut payload = Vec::with_capacity(24);
    payload.push(network);
    payload.extend_from_slice(&port.to_be_bytes());
    write_address(&mut payload, address)?;
    Ok(encode_frame(MUX_STREAM_NEW, STATUS_NEW, 0, &payload))
}

/// Parse a new-stream payload into target info.
pub fn parse_new_stream(payload: &[u8]) -> Result<MuxTarget, Error> {
    if payload.len() < 4 {
        return Err(Error::Protocol("MUX new stream payload too short"));
    }
    let network = payload[0];
    if network != NETWORK_TCP && network != NETWORK_UDP {
        return Err(Error::Protocol("MUX new stream unknown network type"));
    }
    let port = u16::from_be_bytes([payload[1], payload[2]]);
    if port == 0 {
        return Err(Error::Protocol("MUX target port must not be 0"));
    }
    let atyp = payload[3];
    let address = parse_address_from_bytes(atyp, &payload[4..])?;
    Ok(MuxTarget {
        network,
        port,
        address,
    })
}

/// Build a new-stream response frame.
pub fn encode_new_stream_response(assigned_id: u16, status: u8) -> Vec<u8> {
    let mut payload = Vec::with_capacity(3);
    payload.extend_from_slice(&assigned_id.to_be_bytes());
    payload.push(status);
    encode_frame(MUX_STREAM_NEW, STATUS_NEW, 0, &payload)
}

/// Parse a new-stream response payload → (assigned_id, status).
pub fn parse_new_stream_response(payload: &[u8]) -> Result<(u16, u8), Error> {
    if payload.len() < 3 {
        return Err(Error::Protocol("MUX new stream response too short"));
    }
    Ok((u16::from_be_bytes([payload[0], payload[1]]), payload[2]))
}

// ── Data / End / KeepAlive frame helpers ──

/// Build a TCP data frame (STATUS_KEEP | OPTION_DATA).
pub fn encode_data_frame(session_id: u16, data: &[u8]) -> Vec<u8> {
    encode_frame(session_id, STATUS_KEEP, OPTION_DATA, data)
}

/// Build a UDP data frame (STATUS_KEEP | OPTION_DATA) with target prepended.
/// Format: [network:1][port:2][atyp:1][address…][data…]
pub fn encode_udp_data_frame(
    session_id: u16,
    network: u8,
    port: u16,
    address: &Address,
    data: &[u8],
) -> Result<Vec<u8>, Error> {
    let mut payload = Vec::with_capacity(24 + data.len());
    payload.push(network);
    payload.extend_from_slice(&port.to_be_bytes());
    write_address(&mut payload, address)?;
    payload.extend_from_slice(data);
    Ok(encode_frame(session_id, STATUS_KEEP, OPTION_DATA, &payload))
}

/// Build an END frame (terminate the session).
pub fn encode_end_frame(session_id: u16) -> Vec<u8> {
    encode_frame(session_id, STATUS_END, 0, &[])
}

/// Build a KeepAlive frame (session_id=0, status=STATUS_KEEP_ALIVE, empty payload).
pub fn encode_keepalive() -> Vec<u8> {
    encode_frame(MUX_STREAM_NEW, STATUS_KEEP_ALIVE, 0, &[])
}

/// Try to extract target info from a STATUS_KEEP UDP data payload.
/// Returns None if the payload is too short or missing target info.
pub fn parse_udp_target_from_keep(payload: &[u8]) -> Option<MuxTarget> {
    if payload.len() < 4 {
        return None;
    }
    let network = payload[0];
    if network != NETWORK_TCP && network != NETWORK_UDP {
        return None;
    }
    let port = u16::from_be_bytes([payload[1], payload[2]]);
    let atyp = payload[3];
    let address = parse_address_from_bytes(atyp, &payload[4..]).ok()?;
    Some(MuxTarget {
        network,
        port,
        address,
    })
}

// ── Address parsing (internal helper) ──

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

// ── mux client ─────────────────────────────────────────

/// State for one MUX stream on the client side.
#[derive(Debug)]
pub struct MuxClientStream {
    pub id: u16,
}

/// Minimal MUX client — manages stream allocation and frame I/O.
pub struct MuxClient {
    next_id: u16,
    #[cfg(feature = "reality")]
    crypto: Option<crate::mux_crypto::MuxCrypto>,
}

impl Default for MuxClient {
    fn default() -> Self {
        Self::new()
    }
}

impl MuxClient {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            #[cfg(feature = "reality")]
            crypto: None,
        }
    }

    #[cfg(feature = "reality")]
    pub fn with_encryption(master_uuid: &[u8; 16]) -> Self {
        Self {
            next_id: 1,
            crypto: Some(crate::mux_crypto::MuxCrypto::new(master_uuid)),
        }
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

    /// Send a new-stream request (with network type) and return the server-assigned ID.
    pub async fn open_stream<S>(
        &self,
        stream: &mut S,
        network: u8,
        port: u16,
        address: &Address,
    ) -> Result<(u16, MuxClientStream), Error>
    where
        S: AsyncSocket,
    {
        let req = encode_new_stream(network, port, address)?;
        stream
            .write_all(&req)
            .await
            .map_err(|_| Error::Io("failed to write MUX new-stream request"))?;

        let frame = read_mux_frame(stream).await?;
        if frame.session_id != MUX_STREAM_NEW || frame.status != STATUS_NEW {
            return Err(Error::Protocol("expected MUX new-stream response"));
        }
        let (assigned_id, resp_status) = parse_new_stream_response(&frame.payload)?;
        if resp_status != MUX_STATUS_OK {
            return Err(Error::Protocol("MUX server rejected new stream"));
        }

        Ok((assigned_id, MuxClientStream { id: assigned_id }))
    }

    /// Write data to a stream as a STATUS_KEEP frame.
    pub async fn write_data<S>(
        &mut self,
        stream: &mut S,
        sid: u16,
        data: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let payload = self.encrypt_payload_c2s(sid, data);
        let frame = encode_data_frame(sid, &payload);
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX data frame"))
    }

    /// Write an END frame for a stream.
    pub async fn write_end<S>(&mut self, stream: &mut S, sid: u16) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let frame = encode_end_frame(sid);
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX end frame"))
    }

    /// Write a keepalive frame.
    pub async fn write_keepalive<S>(&mut self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let frame = encode_keepalive();
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX keepalive frame"))
    }

    /// Read next incoming frame from server.
    pub async fn recv<S>(&mut self, stream: &mut S) -> Result<MuxFrame, Error>
    where
        S: AsyncSocket,
    {
        let frame = read_mux_frame(stream).await?;
        self.decrypt_frame_s2c(frame)
    }

    fn encrypt_payload_c2s(&mut self, sid: u16, data: &[u8]) -> Vec<u8> {
        #[cfg(not(feature = "reality"))]
        let _ = sid;
        #[cfg(feature = "reality")]
        if sid != MUX_STREAM_NEW {
            if let Some(ref mut crypto) = self.crypto {
                return crypto
                    .encrypt_c2s(sid, data)
                    .unwrap_or_else(|_| data.to_vec());
            }
        }
        data.to_vec()
    }

    fn decrypt_frame_s2c(&mut self, frame: MuxFrame) -> Result<MuxFrame, Error> {
        #[cfg(feature = "reality")]
        if frame.session_id != MUX_STREAM_NEW
            && frame.status != STATUS_KEEP_ALIVE
            && !frame.payload.is_empty()
        {
            if let Some(ref mut crypto) = self.crypto {
                let decrypted = crypto.decrypt_s2c(frame.session_id, &frame.payload)?;
                return Ok(MuxFrame {
                    session_id: frame.session_id,
                    status: frame.status,
                    options: frame.options,
                    payload: decrypted,
                });
            }
        }
        #[cfg(not(feature = "reality"))]
        let _ = frame;
        Ok(frame)
    }
}

// ── mux server ─────────────────────────────────────────

/// MUX server-side handler — reads frames and dispatches.
pub struct MuxServer {
    #[cfg(feature = "reality")]
    crypto: Option<crate::mux_crypto::MuxCrypto>,
}

impl Default for MuxServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MuxServer {
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "reality")]
            crypto: None,
        }
    }

    #[cfg(feature = "reality")]
    pub fn with_encryption(master_uuid: &[u8; 16]) -> Self {
        Self {
            crypto: Some(crate::mux_crypto::MuxCrypto::new(master_uuid)),
        }
    }

    /// Accept a new stream request, allocate an ID, and send response.
    /// Returns `(alloc_id, network, port, address)`.
    pub async fn accept_new_stream<S>(
        &self,
        stream: &mut S,
        alloc_id: u16,
    ) -> Result<(u16, u8, u16, Address), Error>
    where
        S: AsyncSocket,
    {
        let frame = read_mux_frame(stream).await?;
        if frame.session_id != MUX_STREAM_NEW || frame.status != STATUS_NEW {
            return Err(Error::Protocol("expected MUX new-stream request"));
        }

        let target = parse_new_stream(&frame.payload)?;

        let resp = encode_new_stream_response(alloc_id, MUX_STATUS_OK);
        stream
            .write_all(&resp)
            .await
            .map_err(|_| Error::Io("failed to write MUX new-stream response"))?;

        Ok((alloc_id, target.network, target.port, target.address))
    }

    /// Read next frame (with decryption for non-control frames).
    pub async fn recv<S>(&mut self, stream: &mut S) -> Result<MuxFrame, Error>
    where
        S: AsyncSocket,
    {
        let frame = read_mux_frame(stream).await?;
        self.decrypt_frame_c2s(frame)
    }

    /// Write data to a stream as a STATUS_KEEP frame.
    pub async fn write_data<S>(
        &mut self,
        stream: &mut S,
        sid: u16,
        data: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let payload = self.encrypt_payload_s2c(sid, data);
        let frame = encode_data_frame(sid, &payload);
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX data frame"))
    }

    /// Write UDP data to a stream with target info prepended.
    pub async fn write_udp_data<S>(
        &mut self,
        stream: &mut S,
        sid: u16,
        network: u8,
        port: u16,
        address: &Address,
        data: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let payload = self.encrypt_payload_s2c(sid, data);
        let frame = encode_udp_data_frame(sid, network, port, address, &payload)?;
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX UDP data frame"))
    }

    /// Write an END frame for a stream.
    pub async fn write_end<S>(&mut self, stream: &mut S, sid: u16) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let frame = encode_end_frame(sid);
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX end frame"))
    }

    /// Write a keepalive frame.
    pub async fn write_keepalive<S>(&mut self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let frame = encode_keepalive();
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX keepalive frame"))
    }

    fn encrypt_payload_s2c(&mut self, sid: u16, data: &[u8]) -> Vec<u8> {
        #[cfg(not(feature = "reality"))]
        let _ = sid;
        #[cfg(feature = "reality")]
        if sid != MUX_STREAM_NEW {
            if let Some(ref mut crypto) = self.crypto {
                return crypto
                    .encrypt_s2c(sid, data)
                    .unwrap_or_else(|_| data.to_vec());
            }
        }
        data.to_vec()
    }

    fn decrypt_frame_c2s(&mut self, frame: MuxFrame) -> Result<MuxFrame, Error> {
        #[cfg(feature = "reality")]
        if frame.session_id != MUX_STREAM_NEW
            && frame.status != STATUS_KEEP_ALIVE
            && !frame.payload.is_empty()
        {
            if let Some(ref mut crypto) = self.crypto {
                let decrypted = crypto.decrypt_c2s(frame.session_id, &frame.payload)?;
                return Ok(MuxFrame {
                    session_id: frame.session_id,
                    status: frame.status,
                    options: frame.options,
                    payload: decrypted,
                });
            }
        }
        #[cfg(not(feature = "reality"))]
        let _ = frame;
        Ok(frame)
    }
}
