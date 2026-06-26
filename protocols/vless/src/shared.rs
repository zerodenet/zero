use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

#[cfg(feature = "reality")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
#[cfg(feature = "reality")]
use tokio::sync::{broadcast, mpsc};
use zero_core::{Address, Error};
use zero_traits::AsyncSocket;

#[cfg(feature = "reality")]
use crate::outbound::{establish_udp_flow_stream, VlessUdpIdentity};

pub const VLESS_VERSION: u8 = 0x00;

pub(crate) const CMD_TCP: u8 = 0x01;
pub(crate) const CMD_UDP: u8 = 0x02;
pub(crate) const CMD_MUX: u8 = 0x03;

pub(crate) const ATYP_IPV4: u8 = 0x01;
pub(crate) const ATYP_DOMAIN: u8 = 0x02;
pub(crate) const ATYP_IPV6: u8 = 0x03;

pub(crate) async fn read_exact<S>(stream: &mut S, buf: &mut [u8]) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut offset = 0;

    while offset < buf.len() {
        let read = stream
            .read(&mut buf[offset..])
            .await
            .map_err(|_| Error::Io("failed to read from socket"))?;

        if read == 0 {
            return Err(Error::Io("unexpected EOF while reading socket"));
        }

        offset += read;
    }

    Ok(())
}

pub(crate) async fn read_addon<S>(stream: &mut S) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut length = [0_u8; 1];
    read_exact(stream, &mut length).await?;
    let length = length[0] as usize;
    if length == 0 {
        return Ok(());
    }

    let mut addon = vec![0_u8; length];
    read_exact(stream, &mut addon).await
}

pub(crate) async fn read_address<S>(stream: &mut S, atyp: u8) -> Result<Address, Error>
where
    S: AsyncSocket,
{
    match atyp {
        ATYP_IPV4 => {
            let mut bytes = [0_u8; 4];
            read_exact(stream, &mut bytes).await?;
            Ok(Address::Ipv4(bytes))
        }
        ATYP_DOMAIN => {
            let mut length = [0_u8; 1];
            read_exact(stream, &mut length).await?;

            let domain_length = length[0] as usize;
            if domain_length == 0 {
                return Err(Error::Protocol("VLESS domain must not be empty"));
            }

            let mut domain = vec![0_u8; domain_length];
            read_exact(stream, &mut domain).await?;

            let domain = String::from_utf8(domain)
                .map_err(|_| Error::Protocol("VLESS domain is not valid UTF-8"))?;
            Ok(Address::Domain(domain))
        }
        ATYP_IPV6 => {
            let mut bytes = [0_u8; 16];
            read_exact(stream, &mut bytes).await?;
            Ok(Address::Ipv6(bytes))
        }
        _ => Err(Error::Unsupported("VLESS address type is not supported")),
    }
}

pub(crate) fn write_address(buf: &mut Vec<u8>, address: &Address) -> Result<(), Error> {
    match address {
        Address::Ipv4(bytes) => {
            buf.push(ATYP_IPV4);
            buf.extend_from_slice(bytes);
        }
        Address::Ipv6(bytes) => {
            buf.push(ATYP_IPV6);
            buf.extend_from_slice(bytes);
        }
        Address::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.is_empty() {
                return Err(Error::Protocol("VLESS domain must not be empty"));
            }
            if bytes.len() > u8::MAX as usize {
                return Err(Error::Unsupported("VLESS domain is too long"));
            }

            buf.push(ATYP_DOMAIN);
            buf.push(bytes.len() as u8);
            buf.extend_from_slice(bytes);
        }
    }

    Ok(())
}

pub fn parse_uuid(input: &str) -> Result<[u8; 16], Error> {
    let input = input.trim();
    let mut compact = [0_u8; 32];
    let mut offset = 0;

    for (index, byte) in input.bytes().enumerate() {
        if byte == b'-' {
            if !matches!(index, 8 | 13 | 18 | 23) || input.len() != 36 {
                return Err(Error::Config("VLESS UUID is not canonical"));
            }
            continue;
        }

        if offset >= compact.len() {
            return Err(Error::Config("VLESS UUID has too many hex digits"));
        }

        if hex_nibble(byte).is_none() {
            return Err(Error::Config("VLESS UUID contains non-hex digits"));
        }

        compact[offset] = byte;
        offset += 1;
    }

    if offset != compact.len() {
        return Err(Error::Config("VLESS UUID must contain 32 hex digits"));
    }

    let mut uuid = [0_u8; 16];
    for i in 0..16 {
        let high = hex_nibble(compact[i * 2]).expect("hex digit checked");
        let low = hex_nibble(compact[i * 2 + 1]).expect("hex digit checked");
        uuid[i] = (high << 4) | low;
    }

    Ok(uuid)
}

pub fn format_uuid(id: &[u8; 16]) -> String {
    let mut out = String::with_capacity(36);
    for (index, byte) in id.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        out.push(hex_char(byte >> 4));
        out.push(hex_char(byte & 0x0f));
    }
    out
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessUdpPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessUdpFlowPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

impl VlessUdpFlowPacket {
    pub fn new(target: Address, port: u16, payload: Vec<u8>) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(&self.target, self.port, &self.payload)
    }

    pub fn into_parts(self) -> (Address, u16, Vec<u8>) {
        (self.target, self.port, self.payload)
    }
}

#[cfg(feature = "reality")]
pub type VlessUdpFlowResponse = (Address, u16, Vec<u8>);

#[cfg(feature = "reality")]
#[derive(Clone)]
pub struct VlessUdpFlowSender {
    send_tx: mpsc::Sender<VlessUdpFlowPacket>,
}

#[cfg(feature = "reality")]
impl VlessUdpFlowSender {
    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        let packet = VlessUdpFlowPacket::new(target.clone(), port, payload.to_vec());
        let packet_len = packet.encode()?.len();
        self.send_tx
            .send(packet)
            .await
            .map_err(|_| Error::Io("vless udp flow closed"))?;
        Ok(packet_len)
    }
}

#[cfg(feature = "reality")]
pub struct VlessUdpFlowHandle {
    pub sender: VlessUdpFlowSender,
    pub responses: broadcast::Sender<VlessUdpFlowResponse>,
}

#[cfg(feature = "reality")]
pub async fn open_udp_flow<S>(
    mut stream: S,
    session: &zero_core::Session,
    identity: VlessUdpIdentity,
    initial_payload: &[u8],
) -> Result<(VlessUdpFlowHandle, usize), Error>
where
    S: AsyncSocket + AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    establish_udp_flow_stream(&mut stream, session, identity).await?;
    let initial_packet =
        encode_udp_flow_initial_packet(&session.target, session.port, initial_payload)?;
    let initial_packet_len = initial_packet.len();
    AsyncWriteExt::write_all(&mut stream, &initial_packet)
        .await
        .map_err(|_| Error::Io("vless udp flow write"))?;
    AsyncWriteExt::flush(&mut stream)
        .await
        .map_err(|_| Error::Io("vless udp flow flush"))?;
    Ok((spawn_udp_flow(stream, Vec::new()), initial_packet_len))
}

#[cfg(feature = "reality")]
pub fn open_mux_udp_flow<S>(stream: S, initial_packet: Vec<u8>) -> VlessUdpFlowHandle
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    spawn_udp_flow(stream, initial_packet)
}

#[cfg(feature = "reality")]
pub fn encode_udp_flow_initial_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    VlessUdpFlowIo.encode_packet(target, port, payload)
}

#[cfg(feature = "reality")]
fn spawn_udp_flow<S>(stream: S, initial_packet: Vec<u8>) -> VlessUdpFlowHandle
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    let (send_tx, send_rx) = mpsc::channel::<VlessUdpFlowPacket>(32);
    let (responses, _) = broadcast::channel::<VlessUdpFlowResponse>(32);
    spawn_udp_flow_task(stream, initial_packet, send_rx, responses.clone());
    VlessUdpFlowHandle {
        sender: VlessUdpFlowSender { send_tx },
        responses,
    }
}

#[cfg(feature = "reality")]
fn spawn_udp_flow_task<S>(
    mut stream: S,
    initial_packet: Vec<u8>,
    mut send_rx: mpsc::Receiver<VlessUdpFlowPacket>,
    responses: broadcast::Sender<VlessUdpFlowResponse>,
) where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    tokio::spawn(async move {
        if !initial_packet.is_empty() {
            if stream.write_all(&initial_packet).await.is_err() {
                return;
            }
            if stream.flush().await.is_err() {
                return;
            }
        }

        let flow_io = VlessUdpFlowIo;
        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            tokio::select! {
                to_send = send_rx.recv() => {
                    match to_send {
                        Some(packet) => {
                            let (target, port, payload) = packet.into_parts();
                            let encoded = match flow_io.encode_packet(&target, port, &payload) {
                                Ok(encoded) => encoded,
                                Err(_) => break,
                            };
                            if stream.write_all(&encoded).await.is_err() {
                                break;
                            }
                            if stream.flush().await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                read = stream.read(&mut buffer) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Ok(packet) = flow_io.decode_packet(&buffer[..n]) {
                                let _ = responses.send(packet.into_parts());
                            } else {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VlessUdpFlowIo;

impl VlessUdpFlowIo {
    pub fn encode_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    pub fn decode_packet(&self, packet: &[u8]) -> Result<VlessUdpFlowPacket, Error> {
        let packet = decode_udp_flow_packet(packet)?;
        Ok(VlessUdpFlowPacket::new(
            packet.target,
            packet.port,
            packet.payload,
        ))
    }

    pub async fn write_packet<S>(
        &self,
        stream: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        S: AsyncSocket,
    {
        let encoded = self.encode_packet(target, port, payload)?;
        let len = encoded.len();
        stream
            .write_all(&encoded)
            .await
            .map_err(|_| Error::Io("vless udp flow write"))?;
        Ok(len)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VlessUdpFlowCodec;

impl VlessUdpFlowCodec {
    pub fn encode_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    pub fn decode_packet(&self, packet: &[u8]) -> Result<VlessUdpPacket, Error> {
        decode_udp_flow_packet(packet)
    }
}

pub fn parse_udp_packet(packet: &[u8]) -> Result<VlessUdpPacket, Error> {
    if packet.len() < 3 {
        return Err(Error::Protocol("VLESS UDP packet is too short"));
    }

    let mut offset = 0;
    let port = u16::from_be_bytes([packet[offset], packet[offset + 1]]);
    offset += 2;

    let atyp = packet[offset];
    offset += 1;

    let target = match atyp {
        ATYP_IPV4 => {
            if packet.len() < offset + 4 {
                return Err(Error::Protocol("VLESS UDP IPv4 packet is truncated"));
            }
            let mut bytes = [0_u8; 4];
            bytes.copy_from_slice(&packet[offset..offset + 4]);
            offset += 4;
            Address::Ipv4(bytes)
        }
        ATYP_IPV6 => {
            if packet.len() < offset + 16 {
                return Err(Error::Protocol("VLESS UDP IPv6 packet is truncated"));
            }
            let mut bytes = [0_u8; 16];
            bytes.copy_from_slice(&packet[offset..offset + 16]);
            offset += 16;
            Address::Ipv6(bytes)
        }
        ATYP_DOMAIN => {
            if packet.len() < offset + 1 {
                return Err(Error::Protocol("VLESS UDP domain packet is truncated"));
            }
            let len = packet[offset] as usize;
            offset += 1;
            if len == 0 || packet.len() < offset + len {
                return Err(Error::Protocol("VLESS UDP domain packet is truncated"));
            }
            let domain = String::from_utf8(packet[offset..offset + len].to_vec())
                .map_err(|_| Error::Protocol("VLESS UDP domain is not valid UTF-8"))?;
            offset += len;
            Address::Domain(domain)
        }
        _ => {
            return Err(Error::Unsupported(
                "VLESS UDP address type is not supported",
            ));
        }
    };

    Ok(VlessUdpPacket {
        target,
        port,
        payload: packet[offset..].to_vec(),
    })
}

pub fn build_udp_packet(address: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
    let mut packet = Vec::with_capacity(2 + 1 + payload.len());
    packet.extend_from_slice(&port.to_be_bytes());
    write_address(&mut packet, address)?;
    packet.extend_from_slice(payload);
    Ok(packet)
}

// ── VLESS UDP packet v2 ──

/// Flags for v2 UDP packet encoding.
const UDP_V2_HAS_ADDR: u8 = 0x01;

/// V2 magic marker — two zero bytes that can never occur in v1 (port 0 is
/// invalid), making auto-detection unambiguous.
const UDP_V2_MARKER: [u8; 2] = [0x00, 0x00];

/// Parse a VLESS UDP packet, auto-detecting v1 or v2 format.
///
/// v1: `[port:2][atyp:1][addr…][payload]`
/// v2: `[0x00:2][flags:1][port:2][atyp:1][addr… (if flags&1)][payload]`
///
/// When `flags & 1 == 0` (no address in v2), the caller must provide the
/// previously resolved `cached_target` / `cached_port`.
pub fn parse_udp_packet_v2(
    packet: &[u8],
    cached_target: Option<&Address>,
    cached_port: Option<u16>,
) -> Result<VlessUdpPacket, Error> {
    if packet.len() < 3 {
        return Err(Error::Protocol("VLESS UDP packet is too short"));
    }

    // Auto-detect: v2 starts with [0x00, 0x00], v1 starts with port
    if packet[0] == UDP_V2_MARKER[0] && packet[1] == UDP_V2_MARKER[1] {
        parse_udp_v2(packet, cached_target, cached_port)
    } else {
        parse_udp_packet(packet)
    }
}

fn parse_udp_v2(
    packet: &[u8],
    cached_target: Option<&Address>,
    cached_port: Option<u16>,
) -> Result<VlessUdpPacket, Error> {
    let flags = packet[2];
    let has_addr = flags & UDP_V2_HAS_ADDR != 0;

    if has_addr {
        // Full address present: [marker:2][flags:1][port:2][atyp:1][addr…][payload]
        if packet.len() < 8 {
            return Err(Error::Protocol("VLESS UDP v2 packet is too short"));
        }
        let port = u16::from_be_bytes([packet[3], packet[4]]);
        let atyp = packet[5];
        let (target, addr_len) = parse_addr_from_packet(atyp, &packet[6..])?;
        let payload = packet[6 + addr_len..].to_vec();
        Ok(VlessUdpPacket {
            target,
            port,
            payload,
        })
    } else {
        // Address omitted — reuse cached: [marker:2][flags:1][payload]
        let target = cached_target
            .ok_or(Error::Protocol("VLESS UDP v2: no cached target"))?
            .clone();
        let port = cached_port.ok_or(Error::Protocol("VLESS UDP v2: no cached port"))?;
        Ok(VlessUdpPacket {
            target,
            port,
            payload: packet[3..].to_vec(),
        })
    }
}

fn parse_addr_from_packet(atyp: u8, data: &[u8]) -> Result<(Address, usize), Error> {
    match atyp {
        ATYP_IPV4 => {
            if data.len() < 4 {
                return Err(Error::Protocol("VLESS UDP v2 IPv4 address is truncated"));
            }
            let mut bytes = [0_u8; 4];
            bytes.copy_from_slice(&data[..4]);
            Ok((Address::Ipv4(bytes), 4))
        }
        ATYP_IPV6 => {
            if data.len() < 16 {
                return Err(Error::Protocol("VLESS UDP v2 IPv6 address is truncated"));
            }
            let mut bytes = [0_u8; 16];
            bytes.copy_from_slice(&data[..16]);
            Ok((Address::Ipv6(bytes), 16))
        }
        ATYP_DOMAIN => {
            if data.is_empty() {
                return Err(Error::Protocol("VLESS UDP v2 domain packet is truncated"));
            }
            let len = data[0] as usize;
            if len == 0 || data.len() < 1 + len {
                return Err(Error::Protocol("VLESS UDP v2 domain packet is truncated"));
            }
            let domain = String::from_utf8(data[1..1 + len].to_vec())
                .map_err(|_| Error::Protocol("VLESS UDP v2 domain is not valid UTF-8"))?;
            Ok((Address::Domain(domain), 1 + len))
        }
        _ => Err(Error::Unsupported(
            "VLESS UDP v2 address type is not supported",
        )),
    }
}

/// Build a VLESS UDP packet in v2 format.
///
/// When `omit_address` is true and a valid `cached` address/port would be
/// reused by the peer, the address section is omitted, saving 3–21 bytes.
pub fn build_udp_packet_v2(
    address: &Address,
    port: u16,
    payload: &[u8],
    omit_address: bool,
) -> Result<Vec<u8>, Error> {
    if omit_address {
        // [marker:2][flags(0x00):1][payload]
        let mut packet = Vec::with_capacity(3 + payload.len());
        packet.extend_from_slice(&UDP_V2_MARKER);
        packet.push(0x00); // flags: no address
        packet.extend_from_slice(payload);
        Ok(packet)
    } else {
        // [marker:2][flags(0x01):1][port:2][atyp:1][addr…][payload]
        let mut packet = Vec::with_capacity(6 + 1 + payload.len());
        packet.extend_from_slice(&UDP_V2_MARKER);
        packet.push(UDP_V2_HAS_ADDR);
        packet.extend_from_slice(&port.to_be_bytes());
        write_address(&mut packet, address)?;
        packet.extend_from_slice(payload);
        Ok(packet)
    }
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + value - 10),
        _ => unreachable!("nibble value"),
    }
}

pub fn decode_inbound_udp_packet(packet: &[u8]) -> Result<VlessUdpPacket, Error> {
    parse_udp_packet(packet)
}

pub fn encode_udp_response(target: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
    build_udp_packet(target, port, payload)
}

pub fn decode_inbound_udp_datagram(packet: &[u8]) -> Result<VlessUdpPacket, Error> {
    decode_inbound_udp_packet(packet)
}

pub fn encode_inbound_udp_response(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_udp_response(target, port, payload)
}

pub fn encode_inbound_mux_udp_response(
    mux_session_id: u16,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_mux_udp_response(mux_session_id, target, port, payload)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessInboundUdpCodec;

impl VlessInboundUdpCodec {
    pub fn decode_datagram(&self, packet: &[u8]) -> Result<VlessUdpPacket, Error> {
        decode_inbound_udp_datagram(packet)
    }

    pub fn encode_response(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_inbound_udp_response(target, port, payload)
    }

    pub fn encode_mux_response(
        &self,
        mux_session_id: u16,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_inbound_mux_udp_response(mux_session_id, target, port, payload)
    }
}

pub fn decode_udp_flow_packet(packet: &[u8]) -> Result<VlessUdpPacket, Error> {
    parse_udp_packet(packet)
}

pub fn encode_udp_flow_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    build_udp_packet(target, port, payload)
}

pub fn encode_mux_udp_response(
    mux_session_id: u16,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let udp_packet = encode_udp_response(target, port, payload)?;
    Ok(crate::mux::encode_data_frame(mux_session_id, &udp_packet))
}
