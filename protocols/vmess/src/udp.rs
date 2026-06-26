use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc};
use zero_core::{Address, Error, Network, ProtocolType, Session};
use zero_traits::{AsyncSocket, UdpPacketFraming, UdpPacketTunnelProtocol};

use crate::outbound::{VmessOutbound, VmessOutboundSession};
use crate::shared::{parse_address_from_bytes, write_address, VmessCipher, CMD_UDP};
use crate::stream::VmessAeadStream;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmessUdpPayloadMode {
    VmessPacket,
    RawDatagram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmessUdpPayloadState {
    Unknown,
    Mode(VmessUdpPayloadMode),
}

/// Target parameters for a VMess UDP packet tunnel over a connected stream.
#[derive(Debug, Clone, Copy)]
pub struct VmessUdpPacketTunnelTarget<'a> {
    pub session: &'a Session,
    pub uuid: &'a [u8; 16],
    pub cipher: VmessCipher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VmessUdpIdentity {
    pub uuid: [u8; 16],
    pub cipher: VmessCipher,
}

pub fn parse_udp_identity(id: &str, cipher: &str) -> Result<VmessUdpIdentity, Error> {
    let uuid = crate::shared::parse_uuid(id)?;
    let cipher = VmessCipher::from_name(cipher).ok_or(Error::Protocol("vmess unknown cipher"))?;
    Ok(VmessUdpIdentity { uuid, cipher })
}

/// One UDP datagram to encode for a VMess UDP packet tunnel.
#[derive(Debug, Clone, Copy)]
pub struct VmessUdpPacketTarget<'a> {
    pub address: &'a Address,
    pub port: u16,
    pub payload: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmessUdpPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmessUdpFlowPacket {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

impl VmessUdpFlowPacket {
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

pub type VmessUdpFlowResponse = (Address, u16, Vec<u8>);

#[derive(Clone)]
pub struct VmessUdpFlowSender {
    send_tx: mpsc::Sender<VmessUdpFlowPacket>,
}

impl VmessUdpFlowSender {
    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        let packet = VmessUdpFlowPacket::new(target.clone(), port, payload.to_vec());
        let packet_len = packet.encode()?.len();
        self.send_tx
            .send(packet)
            .await
            .map_err(|_| Error::Io("vmess udp flow closed"))?;
        Ok(packet_len)
    }
}

pub struct VmessUdpFlowHandle {
    pub sender: VmessUdpFlowSender,
    pub responses: broadcast::Sender<VmessUdpFlowResponse>,
}

pub async fn open_udp_flow<S>(
    stream: S,
    session: &Session,
    identity: VmessUdpIdentity,
    initial_payload: &[u8],
) -> Result<(VmessUdpFlowHandle, usize), Error>
where
    S: AsyncSocket + AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    let mut stream = establish_udp_flow_stream(stream, session, identity).await?;
    let initial_packet =
        encode_udp_flow_initial_packet(&session.target, session.port, initial_payload)?;
    let initial_packet_len = initial_packet.len();
    AsyncWriteExt::write_all(&mut stream, &initial_packet)
        .await
        .map_err(|_| Error::Io("vmess udp flow write"))?;
    AsyncWriteExt::flush(&mut stream)
        .await
        .map_err(|_| Error::Io("vmess udp flow flush"))?;
    Ok((spawn_udp_flow(stream, Vec::new()), initial_packet_len))
}

pub fn open_mux_udp_flow<S>(stream: S, initial_packet: Vec<u8>) -> VmessUdpFlowHandle
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    spawn_udp_flow(stream, initial_packet)
}

pub fn encode_udp_flow_initial_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    VmessUdpFlowIo.encode_packet(target, port, payload)
}

fn spawn_udp_flow<S>(stream: S, initial_packet: Vec<u8>) -> VmessUdpFlowHandle
where
    S: AsyncRead + AsyncWrite + Send + Sync + Unpin + 'static,
{
    let (send_tx, send_rx) = mpsc::channel::<VmessUdpFlowPacket>(32);
    let (responses, _) = broadcast::channel::<VmessUdpFlowResponse>(32);
    spawn_udp_flow_task(stream, initial_packet, send_rx, responses.clone());
    VmessUdpFlowHandle {
        sender: VmessUdpFlowSender { send_tx },
        responses,
    }
}

fn spawn_udp_flow_task<S>(
    mut stream: S,
    initial_packet: Vec<u8>,
    mut send_rx: mpsc::Receiver<VmessUdpFlowPacket>,
    responses: broadcast::Sender<VmessUdpFlowResponse>,
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

        let flow_io = VmessUdpFlowIo;
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
pub struct VmessUdpFlowIo;

impl VmessUdpFlowIo {
    pub fn encode_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    pub fn decode_packet(&self, packet: &[u8]) -> Result<VmessUdpFlowPacket, Error> {
        let packet = decode_udp_flow_packet(packet)?;
        Ok(VmessUdpFlowPacket::new(
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
            .map_err(|_| Error::Io("vmess udp flow write"))?;
        Ok(len)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VmessUdpFlowCodec;

impl VmessUdpFlowCodec {
    pub fn encode_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_flow_packet(target, port, payload)
    }

    pub fn decode_packet(&self, packet: &[u8]) -> Result<VmessUdpPacket, Error> {
        decode_udp_flow_packet(packet)
    }
}

pub struct VmessInboundUdpPayload {
    pub state: VmessUdpPayloadState,
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

impl VmessOutbound {
    pub async fn establish_udp_packet_session<S>(
        &self,
        stream: &mut S,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
    ) -> Result<VmessOutboundSession, Error>
    where
        S: AsyncSocket,
    {
        let udp_session = Session::new(
            session.id,
            session.target.clone(),
            session.port,
            Network::Udp,
            ProtocolType::Vmess,
        );
        self.establish_command_session(stream, &udp_session, uuid, cipher, CMD_UDP)
            .await
    }
}

impl<'a> UdpPacketTunnelProtocol<VmessUdpPacketTunnelTarget<'a>> for VmessOutbound {
    type Error = Error;

    async fn establish_udp_packet_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VmessUdpPacketTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_udp_packet_session(stream, target.session, target.uuid, target.cipher)
            .await
            .map(|_| ())
    }
}

impl<'a> UdpPacketFraming<VmessUdpPacketTarget<'a>> for VmessOutbound {
    type Error = Error;
    type Decoded = VmessUdpPacket;

    fn encode_udp_packet(&self, packet: &VmessUdpPacketTarget<'a>) -> Result<Vec<u8>, Self::Error> {
        build_udp_packet(packet.address, packet.port, packet.payload)
    }

    fn decode_udp_packet(&self, packet: &[u8]) -> Result<Self::Decoded, Self::Error> {
        parse_udp_packet(packet)
    }
}

impl<S> VmessAeadStream<S> {
    pub async fn establish_udp_outbound(
        mut inner: S,
        outbound: &VmessOutbound,
        session: &Session,
        uuid: &[u8; 16],
        cipher: VmessCipher,
    ) -> Result<Self, Error>
    where
        S: AsyncSocket,
    {
        let vmess_session = outbound
            .establish_udp_packet_session(&mut inner, session, uuid, cipher)
            .await?;
        VmessAeadStream::outbound(inner, vmess_session)
    }
}

pub async fn establish_udp_outbound_stream<S>(
    stream: S,
    session: &Session,
    uuid: &[u8; 16],
    cipher: VmessCipher,
) -> Result<VmessAeadStream<S>, Error>
where
    S: AsyncSocket,
{
    VmessAeadStream::establish_udp_outbound(stream, &VmessOutbound, session, uuid, cipher).await
}

pub async fn establish_udp_flow_stream<S>(
    stream: S,
    session: &Session,
    identity: VmessUdpIdentity,
) -> Result<VmessAeadStream<S>, Error>
where
    S: AsyncSocket,
{
    establish_udp_outbound_stream(stream, session, &identity.uuid, identity.cipher).await
}

pub fn build_udp_packet(address: &Address, port: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
    let mut body = Vec::with_capacity(8 + payload.len());
    write_address(&mut body, address)?;
    body.extend_from_slice(&port.to_be_bytes());
    body.extend_from_slice(payload);

    if body.len() > u16::MAX as usize {
        return Err(Error::Protocol("vmess udp packet too large"));
    }

    let mut packet = Vec::with_capacity(2 + body.len());
    packet.extend_from_slice(&(body.len() as u16).to_be_bytes());
    packet.extend_from_slice(&body);
    Ok(packet)
}

pub fn encode_udp_response(
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    match mode {
        VmessUdpPayloadMode::VmessPacket => build_udp_packet(target, port, payload),
        VmessUdpPayloadMode::RawDatagram => Ok(payload.to_vec()),
    }
}

pub fn encode_udp_flow_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    build_udp_packet(target, port, payload)
}

pub fn decode_udp_flow_packet(packet: &[u8]) -> Result<VmessUdpPacket, Error> {
    parse_udp_packet(packet)
}

pub fn encode_mux_udp_response(
    mux_session_id: u16,
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let payload = encode_udp_response(mode, target, port, payload)?;
    crate::mux::encode_keep_stream(mux_session_id, &payload)
}

pub fn encode_inbound_udp_response(
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_udp_response(mode, target, port, payload)
}

pub fn encode_inbound_mux_udp_response(
    mux_session_id: u16,
    mode: VmessUdpPayloadMode,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_mux_udp_response(mux_session_id, mode, target, port, payload)
}

pub fn decode_inbound_udp_payload(
    state: VmessUdpPayloadState,
    default_target: &Address,
    default_port: u16,
    payload: &[u8],
) -> Result<VmessInboundUdpPayload, Error> {
    match state {
        VmessUdpPayloadState::Unknown => match parse_udp_packet(payload) {
            Ok(packet) => Ok(VmessInboundUdpPayload {
                state: VmessUdpPayloadState::Mode(VmessUdpPayloadMode::VmessPacket),
                target: packet.target,
                port: packet.port,
                payload: packet.payload,
            }),
            Err(_) => Ok(VmessInboundUdpPayload {
                state: VmessUdpPayloadState::Mode(VmessUdpPayloadMode::RawDatagram),
                target: default_target.clone(),
                port: default_port,
                payload: payload.to_vec(),
            }),
        },
        VmessUdpPayloadState::Mode(VmessUdpPayloadMode::VmessPacket) => {
            let packet = parse_udp_packet(payload)?;
            Ok(VmessInboundUdpPayload {
                state,
                target: packet.target,
                port: packet.port,
                payload: packet.payload,
            })
        }
        VmessUdpPayloadState::Mode(VmessUdpPayloadMode::RawDatagram) => {
            Ok(VmessInboundUdpPayload {
                state,
                target: default_target.clone(),
                port: default_port,
                payload: payload.to_vec(),
            })
        }
    }
}

pub fn decode_inbound_udp_datagram(
    state: VmessUdpPayloadState,
    default_target: &Address,
    default_port: u16,
    payload: &[u8],
) -> Result<VmessInboundUdpPayload, Error> {
    decode_inbound_udp_payload(state, default_target, default_port, payload)
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VmessInboundUdpCodec;

impl VmessInboundUdpCodec {
    pub fn encode_response(
        &self,
        mode: VmessUdpPayloadMode,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_inbound_udp_response(mode, target, port, payload)
    }

    pub fn encode_mux_response(
        &self,
        mux_session_id: u16,
        mode: VmessUdpPayloadMode,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_inbound_mux_udp_response(mux_session_id, mode, target, port, payload)
    }

    pub fn decode_datagram(
        &self,
        state: VmessUdpPayloadState,
        default_target: &Address,
        default_port: u16,
        payload: &[u8],
    ) -> Result<VmessInboundUdpPayload, Error> {
        decode_inbound_udp_datagram(state, default_target, default_port, payload)
    }
}

pub fn parse_udp_packet(packet: &[u8]) -> Result<VmessUdpPacket, Error> {
    if packet.len() < 2 {
        return Err(Error::Protocol("vmess udp packet too short"));
    }

    let body_len = u16::from_be_bytes([packet[0], packet[1]]) as usize;
    if packet.len() < 2 + body_len {
        return Err(Error::Protocol("vmess udp packet truncated"));
    }
    let body = &packet[2..2 + body_len];

    let (target, offset) = parse_address_body(body)?;
    if body.len() < offset + 2 {
        return Err(Error::Protocol("vmess udp packet missing port"));
    }
    let port = u16::from_be_bytes([body[offset], body[offset + 1]]);
    let payload = body[offset + 2..].to_vec();

    Ok(VmessUdpPacket {
        target,
        port,
        payload,
    })
}

fn parse_address_body(body: &[u8]) -> Result<(Address, usize), Error> {
    if body.is_empty() {
        return Err(Error::Protocol("vmess udp empty address body"));
    }

    let atyp = body[0];
    match atyp {
        0x01 => {
            if body.len() < 5 {
                return Err(Error::Protocol("vmess udp truncated ipv4"));
            }
            Ok((parse_address_from_bytes(atyp, &body[1..5])?, 5))
        }
        0x02 => {
            if body.len() < 2 {
                return Err(Error::Protocol("vmess udp truncated domain length"));
            }
            let len = body[1] as usize;
            let end = 2 + len;
            if body.len() < end {
                return Err(Error::Protocol("vmess udp truncated domain"));
            }
            Ok((parse_address_from_bytes(atyp, &body[1..end])?, end))
        }
        0x03 => {
            if body.len() < 17 {
                return Err(Error::Protocol("vmess udp truncated ipv6"));
            }
            Ok((parse_address_from_bytes(atyp, &body[1..17])?, 17))
        }
        _ => Err(Error::Protocol("vmess udp unknown address type")),
    }
}
