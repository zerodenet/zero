use alloc::string::String;
#[cfg(feature = "reality")]
use alloc::vec;
use alloc::vec::Vec;

use zero_core::{Address, Error, InboundUdpDispatch, ProtocolType};
#[cfg(feature = "reality")]
use zero_core::{MuxUdpDecodeFailure, MuxUdpResponder, StreamUdpResponder};
use zero_traits::AsyncSocket;

use crate::shared::{write_address, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6};

#[cfg(feature = "reality")]
pub use crate::outbound::{
    establish_udp_flow, establish_udp_flow_with_initial_packet, spawn_udp_flow,
    VlessEstablishedUdpFlow, VlessEstablishedUdpFlowHandle, VlessInitialUdpFlowPacket,
    VlessMuxInitialUdpFlowPacket, VlessUdpFlowConnection, VlessUdpFlowHandle, VlessUdpFlowResponse,
    VlessUdpFlowResponseReceiver, VlessUdpFlowSession,
};
pub use crate::outbound::{
    establish_udp_flow_stream, establish_udp_packet_tunnel, parse_udp_identity,
    udp_flow_config_from_config, VlessUdpFlowConfig, VlessUdpIdentity, VlessUdpMuxOpenIdentity,
    VlessUdpPacketTarget, VlessUdpPacketTunnelTarget,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessUdpPacket {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl VlessUdpPacket {
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
pub struct VlessInboundUdpRequest {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessInboundUdpDispatchParts {
    target: Address,
    port: u16,
    payload: Vec<u8>,
    client_session_id: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
pub struct VlessInboundUdpClientResponse<'a> {
    target: &'a Address,
    port: u16,
    payload: &'a [u8],
}

impl<'a> VlessInboundUdpClientResponse<'a> {
    pub fn new(target: &'a Address, port: u16, payload: &'a [u8]) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    fn target(&self) -> &'a Address {
        self.target
    }

    fn port(&self) -> u16 {
        self.port
    }

    fn payload(&self) -> &'a [u8] {
        self.payload
    }
}

impl VlessInboundUdpRequest {
    fn from_packet(packet: VlessUdpPacket) -> Self {
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

    pub fn into_dispatch_parts(self) -> VlessInboundUdpDispatchParts {
        let (target, port, payload) = self.into_parts();
        VlessInboundUdpDispatchParts {
            target,
            port,
            payload,
            client_session_id: None,
        }
    }
}

impl VlessInboundUdpDispatchParts {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vless
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
        InboundUdpDispatch::new(
            ProtocolType::Vless,
            self.target,
            self.port,
            self.payload,
            self.client_session_id,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessUdpFlowPacket {
    target: Address,
    port: u16,
    payload: Vec<u8>,
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

#[cfg(feature = "reality")]
pub fn encode_udp_flow_initial_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    VlessUdpFlowIo.encode_packet(target, port, payload)
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
        let (target, port, payload) = packet.into_parts();
        Ok(VlessUdpFlowPacket::new(target, port, payload))
    }

    pub fn encoded_packet_len(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.encode_packet(target, port, payload)
            .map(|packet| packet.len())
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

    #[cfg(feature = "reality")]
    pub async fn write_packet_tokio<S>(
        &self,
        stream: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        S: tokio::io::AsyncWrite + Unpin,
    {
        let encoded = self.encode_packet(target, port, payload)?;
        let len = encoded.len();
        tokio::io::AsyncWriteExt::write_all(stream, &encoded)
            .await
            .map_err(|_| Error::Io("vless udp flow write"))?;
        tokio::io::AsyncWriteExt::flush(stream)
            .await
            .map_err(|_| Error::Io("vless udp flow flush"))?;
        Ok(len)
    }

    #[cfg(feature = "reality")]
    pub async fn read_packet_tokio<S>(
        &self,
        stream: &mut S,
        buffer: &mut [u8],
    ) -> Result<Option<VlessUdpFlowPacket>, Error>
    where
        S: tokio::io::AsyncRead + Unpin,
    {
        let n = tokio::io::AsyncReadExt::read(stream, buffer)
            .await
            .map_err(|_| Error::Io("vless udp flow read"))?;
        if n == 0 {
            return Ok(None);
        }
        self.decode_packet(&buffer[..n]).map(Some)
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

pub(crate) fn parse_udp_packet(packet: &[u8]) -> Result<VlessUdpPacket, Error> {
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

pub(crate) fn build_udp_packet(
    address: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let mut packet = Vec::with_capacity(2 + 1 + payload.len());
    packet.extend_from_slice(&port.to_be_bytes());
    write_address(&mut packet, address)?;
    packet.extend_from_slice(payload);
    Ok(packet)
}

const UDP_V2_HAS_ADDR: u8 = 0x01;
const UDP_V2_MARKER: [u8; 2] = [0x00, 0x00];

pub(crate) fn parse_udp_packet_v2(
    packet: &[u8],
    cached_target: Option<&Address>,
    cached_port: Option<u16>,
) -> Result<VlessUdpPacket, Error> {
    if packet.len() < 3 {
        return Err(Error::Protocol("VLESS UDP packet is too short"));
    }

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

pub(crate) fn build_udp_packet_v2(
    address: &Address,
    port: u16,
    payload: &[u8],
    omit_address: bool,
) -> Result<Vec<u8>, Error> {
    if omit_address {
        let mut packet = Vec::with_capacity(3 + payload.len());
        packet.extend_from_slice(&UDP_V2_MARKER);
        packet.push(0x00);
        packet.extend_from_slice(payload);
        Ok(packet)
    } else {
        let mut packet = Vec::with_capacity(6 + 1 + payload.len());
        packet.extend_from_slice(&UDP_V2_MARKER);
        packet.push(UDP_V2_HAS_ADDR);
        packet.extend_from_slice(&port.to_be_bytes());
        write_address(&mut packet, address)?;
        packet.extend_from_slice(payload);
        Ok(packet)
    }
}

pub(crate) fn decode_inbound_udp_packet(packet: &[u8]) -> Result<VlessUdpPacket, Error> {
    parse_udp_packet(packet)
}

pub(crate) fn encode_udp_response(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    build_udp_packet(target, port, payload)
}

fn decode_inbound_udp_datagram(packet: &[u8]) -> Result<VlessUdpPacket, Error> {
    decode_inbound_udp_packet(packet)
}

fn encode_inbound_udp_response(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_udp_response(target, port, payload)
}

fn encode_inbound_mux_udp_response(
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
    pub fn decode_request(&self, packet: &[u8]) -> Result<VlessInboundUdpRequest, Error> {
        self.decode_datagram(packet)
            .map(VlessInboundUdpRequest::from_packet)
    }

    pub fn decode_dispatch_parts(
        &self,
        packet: &[u8],
    ) -> Result<VlessInboundUdpDispatchParts, Error> {
        self.decode_request(packet)
            .map(VlessInboundUdpRequest::into_dispatch_parts)
    }

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

    #[cfg(feature = "reality")]
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
        let packet = self.encode_response(target, port, payload)?;
        let len = packet.len();
        tokio::io::AsyncWriteExt::write_all(writer, &packet)
            .await
            .map_err(|_| Error::Io("failed to write VLESS UDP response"))?;
        tokio::io::AsyncWriteExt::flush(writer)
            .await
            .map_err(|_| Error::Io("failed to flush VLESS UDP response"))?;
        Ok(len)
    }

    #[cfg(feature = "reality")]
    pub async fn write_client_response_tokio<W>(
        &self,
        writer: &mut W,
        response: VlessInboundUdpClientResponse<'_>,
    ) -> Result<usize, Error>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.write_response_tokio(
            writer,
            response.target(),
            response.port(),
            response.payload(),
        )
        .await
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

    #[cfg(feature = "reality")]
    pub fn send_mux_response(
        &self,
        writer: &crate::mux::VlessInboundMuxWriter,
        mux_session_id: u16,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        let frame = self.encode_mux_response(mux_session_id, target, port, payload)?;
        writer.frame(mux_session_id, frame)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessInboundUdpSession {
    codec: VlessInboundUdpCodec,
}

#[cfg(feature = "reality")]
pub struct VlessInboundUdpResponder {
    session: VlessInboundUdpSession,
    read_buf: Vec<u8>,
}

#[cfg(feature = "reality")]
pub struct VlessInboundMuxUdpResponder {
    session: VlessInboundUdpSession,
    writer: crate::mux::VlessInboundMuxWriter,
    mux_session_id: u16,
}

impl VlessInboundUdpSession {
    pub fn new() -> Self {
        Self {
            codec: VlessInboundUdpCodec,
        }
    }

    pub fn decode_request(&self, packet: &[u8]) -> Result<VlessInboundUdpRequest, Error> {
        self.codec.decode_request(packet)
    }

    pub fn decode_dispatch_parts(
        &self,
        packet: &[u8],
    ) -> Result<VlessInboundUdpDispatchParts, Error> {
        self.codec.decode_dispatch_parts(packet)
    }

    pub fn decode_mux_dispatch_parts(
        &self,
        payload: &[u8],
    ) -> Result<VlessInboundUdpDispatchParts, Error> {
        self.decode_dispatch_parts(payload)
    }

    pub fn decode_mux_inbound_dispatch(&self, payload: &[u8]) -> Result<InboundUdpDispatch, Error> {
        self.decode_mux_dispatch_parts(payload)
            .map(VlessInboundUdpDispatchParts::into_inbound_dispatch)
    }

    #[cfg(feature = "reality")]
    pub async fn read_dispatch_parts_tokio<R>(
        &self,
        reader: &mut R,
        buf: &mut [u8],
    ) -> Result<Option<VlessInboundUdpDispatchParts>, Error>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let n = tokio::io::AsyncReadExt::read(reader, buf)
            .await
            .map_err(|_| Error::Io("failed to read VLESS UDP request"))?;
        if n == 0 {
            return Ok(None);
        }
        self.decode_dispatch_parts(&buf[..n]).map(Some)
    }

    #[cfg(feature = "reality")]
    pub async fn read_inbound_dispatch_tokio<R>(
        &self,
        reader: &mut R,
        buf: &mut [u8],
    ) -> Result<Option<InboundUdpDispatch>, Error>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        self.read_dispatch_parts_tokio(reader, buf)
            .await
            .map(|parts| parts.map(VlessInboundUdpDispatchParts::into_inbound_dispatch))
    }

    #[cfg(feature = "reality")]
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
        self.codec
            .write_response_tokio(writer, target, port, payload)
            .await
    }

    #[cfg(feature = "reality")]
    pub async fn write_client_response_tokio<W>(
        &self,
        writer: &mut W,
        response: VlessInboundUdpClientResponse<'_>,
    ) -> Result<usize, Error>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.codec
            .write_client_response_tokio(writer, response)
            .await
    }

    #[cfg(feature = "reality")]
    pub async fn write_client_response_for_target_tokio<W>(
        &self,
        writer: &mut W,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.write_client_response_tokio(
            writer,
            VlessInboundUdpClientResponse::new(target, port, payload),
        )
        .await
    }

    #[cfg(feature = "reality")]
    pub fn send_mux_response(
        &self,
        writer: &crate::mux::VlessInboundMuxWriter,
        mux_session_id: u16,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.codec
            .send_mux_response(writer, mux_session_id, target, port, payload)
    }

    #[cfg(feature = "reality")]
    pub fn send_mux_client_response(
        &self,
        writer: &crate::mux::VlessInboundMuxWriter,
        mux_session_id: u16,
        response: VlessInboundUdpClientResponse<'_>,
    ) -> Result<usize, Error> {
        self.send_mux_response(
            writer,
            mux_session_id,
            response.target(),
            response.port(),
            response.payload(),
        )
    }

    #[cfg(feature = "reality")]
    pub fn send_mux_client_response_for_target(
        &self,
        writer: &crate::mux::VlessInboundMuxWriter,
        mux_session_id: u16,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.send_mux_client_response(
            writer,
            mux_session_id,
            VlessInboundUdpClientResponse::new(target, port, payload),
        )
    }
}

#[cfg(feature = "reality")]
impl VlessInboundUdpResponder {
    pub fn new(session: VlessInboundUdpSession) -> Self {
        Self {
            session,
            read_buf: vec![0_u8; 64 * 1024],
        }
    }

    pub async fn read_inbound_dispatch_tokio<R>(
        &mut self,
        reader: &mut R,
    ) -> Result<Option<InboundUdpDispatch>, Error>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        self.session
            .read_inbound_dispatch_tokio(reader, &mut self.read_buf)
            .await
    }

    pub async fn write_response_for_target_tokio<W>(
        &self,
        writer: &mut W,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        self.session
            .write_client_response_for_target_tokio(writer, target, port, payload)
            .await
    }
}

#[cfg(feature = "reality")]
impl<S> StreamUdpResponder<S> for VlessInboundUdpResponder
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Unpin,
{
    async fn read_inbound_dispatch(
        &mut self,
        client: &mut S,
    ) -> Result<Option<InboundUdpDispatch>, Error> {
        self.read_inbound_dispatch_tokio(client).await
    }

    async fn write_response_for_target(
        &mut self,
        client: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.write_response_for_target_tokio(client, target, port, payload)
            .await
    }
}

#[cfg(feature = "reality")]
impl VlessInboundMuxUdpResponder {
    pub fn new(
        session: VlessInboundUdpSession,
        writer: crate::mux::VlessInboundMuxWriter,
        mux_session_id: u16,
    ) -> Self {
        Self {
            session,
            writer,
            mux_session_id,
        }
    }

    pub fn decode_inbound_dispatch(&self, payload: &[u8]) -> Result<InboundUdpDispatch, Error> {
        self.session.decode_mux_inbound_dispatch(payload)
    }

    pub fn write_response_for_target(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.session.send_mux_client_response_for_target(
            &self.writer,
            self.mux_session_id,
            target,
            port,
            payload,
        )
    }

    pub fn end_inbound_stream(&self) -> Result<usize, Error> {
        self.writer.end_inbound_stream(self.mux_session_id)
    }
}

#[cfg(feature = "reality")]
impl MuxUdpResponder for VlessInboundMuxUdpResponder {
    fn decode_inbound_dispatch(&mut self, payload: &[u8]) -> Result<InboundUdpDispatch, Error> {
        VlessInboundMuxUdpResponder::decode_inbound_dispatch(self, payload)
    }

    fn write_response_for_target(
        &mut self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        VlessInboundMuxUdpResponder::write_response_for_target(self, target, port, payload)
    }

    fn end_inbound_stream(&mut self) -> Result<usize, Error> {
        VlessInboundMuxUdpResponder::end_inbound_stream(self)
    }

    fn decode_failure(&self) -> MuxUdpDecodeFailure {
        MuxUdpDecodeFailure::Continue
    }
}

pub(crate) fn decode_udp_flow_packet(packet: &[u8]) -> Result<VlessUdpPacket, Error> {
    parse_udp_packet(packet)
}

pub(crate) fn encode_udp_flow_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    build_udp_packet(target, port, payload)
}

pub(crate) fn encode_mux_udp_response(
    mux_session_id: u16,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let udp_packet = encode_udp_response(target, port, payload)?;
    Ok(crate::mux::encode_data_frame(mux_session_id, &udp_packet))
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessUdpPacketV2Codec;

impl VlessUdpPacketV2Codec {
    pub fn decode_packet(
        &self,
        packet: &[u8],
        cached_target: Option<&Address>,
        cached_port: Option<u16>,
    ) -> Result<VlessUdpPacket, Error> {
        parse_udp_packet_v2(packet, cached_target, cached_port)
    }

    pub fn encode_packet(
        &self,
        address: &Address,
        port: u16,
        payload: &[u8],
        omit_address: bool,
    ) -> Result<Vec<u8>, Error> {
        build_udp_packet_v2(address, port, payload, omit_address)
    }
}

impl crate::inbound::VlessInbound {
    pub fn udp_session(&self) -> VlessInboundUdpSession {
        VlessInboundUdpSession::new()
    }

    #[cfg(feature = "reality")]
    pub fn udp_responder(&self) -> VlessInboundUdpResponder {
        VlessInboundUdpResponder::new(self.udp_session())
    }

    #[cfg(feature = "reality")]
    pub fn mux_udp_responder(
        &self,
        writer: crate::mux::VlessInboundMuxWriter,
        mux_session_id: u16,
    ) -> VlessInboundMuxUdpResponder {
        VlessInboundMuxUdpResponder::new(self.udp_session(), writer, mux_session_id)
    }

    #[cfg(feature = "reality")]
    pub async fn accept_udp_session<S>(
        &self,
        stream: &mut S,
    ) -> Result<VlessInboundUdpResponder, Error>
    where
        S: AsyncSocket,
    {
        self.send_response(stream).await?;
        Ok(self.udp_responder())
    }
}
