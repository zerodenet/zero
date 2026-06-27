use tokio::sync::{broadcast, mpsc, oneshot};
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

pub fn encode_udp_flow_initial_packet(
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    VmessUdpFlowIo.encode_packet(target, port, payload)
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
            .map_err(|_| Error::Io("vmess udp flow write"))?;
        Ok(len)
    }

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
            .map_err(|_| Error::Io("vmess udp flow write"))?;
        tokio::io::AsyncWriteExt::flush(stream)
            .await
            .map_err(|_| Error::Io("vmess udp flow flush"))?;
        Ok(len)
    }

    pub async fn read_packet_tokio<S>(
        &self,
        stream: &mut S,
        buffer: &mut [u8],
    ) -> Result<Option<VmessUdpFlowPacket>, Error>
    where
        S: tokio::io::AsyncRead + Unpin,
    {
        let n = tokio::io::AsyncReadExt::read(stream, buffer)
            .await
            .map_err(|_| Error::Io("vmess udp flow read"))?;
        if n == 0 {
            return Ok(None);
        }
        self.decode_packet(&buffer[..n]).map(Some)
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

#[derive(Debug, Clone, Copy, Default)]
pub struct VmessEstablishedUdpFlow {
    io: VmessUdpFlowIo,
}

pub type VmessUdpFlowResponse = (Address, u16, Vec<u8>);

type VmessUdpFlowResponses = broadcast::Sender<VmessUdpFlowResponse>;

pub type VmessUdpFlowResponseReceiver = broadcast::Receiver<VmessUdpFlowResponse>;

struct VmessUdpFlowSend {
    packet: zero_core::UdpFlowPacket,
    result_tx: oneshot::Sender<Result<usize, Error>>,
}

#[derive(Clone)]
pub struct VmessInitialUdpFlowPacket {
    packet: zero_core::UdpFlowPacket,
}

impl VmessInitialUdpFlowPacket {
    pub fn from_parts(target: &Address, port: u16, payload: &[u8]) -> Self {
        Self {
            packet: zero_core::UdpFlowPacket::from_parts(target, port, payload),
        }
    }

    pub fn encoded_len(&self, flow: &VmessEstablishedUdpFlow) -> Result<usize, Error> {
        flow.encoded_packet_len(&self.packet.target, self.packet.port, &self.packet.payload)
    }

    pub fn encode(&self, flow: &VmessEstablishedUdpFlow) -> Result<Vec<u8>, Error> {
        flow.initial_packet(&self.packet.target, self.packet.port, &self.packet.payload)
    }

    fn write_target(&self) -> (&Address, u16, &[u8]) {
        (&self.packet.target, self.packet.port, &self.packet.payload)
    }
}

#[derive(Clone)]
struct VmessUdpFlowSender {
    send_tx: mpsc::Sender<VmessUdpFlowSend>,
}

pub struct VmessUdpFlowHandle {
    sender: VmessUdpFlowSender,
    responses: VmessUdpFlowResponses,
}

pub struct VmessEstablishedUdpFlowHandle {
    pub handle: VmessUdpFlowHandle,
    pub initial_packet_len: usize,
}

#[derive(Clone)]
pub struct VmessUdpFlowSession {
    sender: VmessUdpFlowSender,
    responses: VmessUdpFlowResponses,
}

impl VmessUdpFlowSession {
    pub fn new(handle: VmessUdpFlowHandle) -> Self {
        Self {
            sender: handle.sender,
            responses: handle.responses,
        }
    }

    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        self.sender.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> VmessUdpFlowResponseReceiver {
        self.responses.subscribe()
    }
}

impl VmessUdpFlowSender {
    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        let packet = zero_core::UdpFlowPacket::from_parts(target, port, payload);
        let (result_tx, result_rx) = oneshot::channel();
        self.send_tx
            .send(VmessUdpFlowSend { packet, result_tx })
            .await
            .map_err(|_| Error::Io("vmess udp flow closed"))?;
        result_rx
            .await
            .map_err(|_| Error::Io("vmess udp flow closed"))?
    }
}

impl VmessEstablishedUdpFlow {
    pub fn encode_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.io.encode_packet(target, port, payload)
    }

    pub fn encoded_packet_len(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.io.encoded_packet_len(target, port, payload)
    }

    pub fn initial_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.io.encode_packet(target, port, payload)
    }

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
        self.io
            .write_packet_tokio(stream, target, port, payload)
            .await
    }

    pub async fn read_packet_tokio<S>(
        &self,
        stream: &mut S,
        buffer: &mut [u8],
    ) -> Result<Option<VmessUdpFlowPacket>, Error>
    where
        S: tokio::io::AsyncRead + Unpin,
    {
        self.io.read_packet_tokio(stream, buffer).await
    }
}

pub fn spawn_udp_flow<S>(
    stream: S,
    initial_packet: Option<VmessInitialUdpFlowPacket>,
    flow_io: VmessEstablishedUdpFlow,
) -> VmessUdpFlowHandle
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
{
    let (send_tx, send_rx) = mpsc::channel::<VmessUdpFlowSend>(32);
    let (responses, _) = broadcast::channel::<VmessUdpFlowResponse>(32);
    spawn_udp_flow_task(stream, initial_packet, send_rx, responses.clone(), flow_io);
    VmessUdpFlowHandle {
        sender: VmessUdpFlowSender { send_tx },
        responses,
    }
}

pub fn start_udp_flow_with_initial_packet<S>(
    stream: S,
    target: &Address,
    port: u16,
    initial_payload: &[u8],
) -> Result<VmessEstablishedUdpFlowHandle, Error>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
{
    let flow_io = VmessEstablishedUdpFlow::default();
    let initial_packet = VmessInitialUdpFlowPacket::from_parts(target, port, initial_payload);
    let initial_packet_len = initial_packet.encoded_len(&flow_io)?;
    let handle = spawn_udp_flow(stream, Some(initial_packet), flow_io);

    Ok(VmessEstablishedUdpFlowHandle {
        handle,
        initial_packet_len,
    })
}

fn spawn_udp_flow_task<S>(
    mut stream: S,
    initial_packet: Option<VmessInitialUdpFlowPacket>,
    mut send_rx: mpsc::Receiver<VmessUdpFlowSend>,
    responses: VmessUdpFlowResponses,
    flow_io: VmessEstablishedUdpFlow,
) where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
{
    tokio::spawn(async move {
        if let Some(packet) = initial_packet {
            let (target, port, payload) = packet.write_target();
            if flow_io
                .write_packet_tokio(&mut stream, target, port, payload)
                .await
                .is_err()
            {
                return;
            }
        }

        let mut buffer = vec![0_u8; 64 * 1024];
        loop {
            tokio::select! {
                to_send = send_rx.recv() => {
                    match to_send {
                        Some(request) => {
                            let result = flow_io
                                .write_packet_tokio(
                                    &mut stream,
                                    &request.packet.target,
                                    request.packet.port,
                                    &request.packet.payload,
                                )
                                .await;
                            let should_break = result.is_err();
                            let _ = request.result_tx.send(result);
                            if should_break {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                read = flow_io.read_packet_tokio(&mut stream, &mut buffer) => {
                    match read {
                        Ok(Some(packet)) => {
                            let _ = responses.send(packet.into_parts());
                        }
                        Ok(None) => break,
                        Err(_) => break,
                    }
                }
            }
        }
    });
}

pub async fn establish_udp_flow<S>(
    stream: S,
    session: &Session,
    identity: VmessUdpIdentity,
) -> Result<(VmessAeadStream<S>, VmessEstablishedUdpFlow), Error>
where
    S: AsyncSocket,
{
    let stream = establish_udp_flow_stream(stream, session, identity).await?;
    Ok((stream, VmessEstablishedUdpFlow { io: VmessUdpFlowIo }))
}

pub async fn establish_udp_flow_with_initial_packet<S>(
    stream: S,
    session: &Session,
    identity: VmessUdpIdentity,
    initial_payload: &[u8],
) -> Result<VmessEstablishedUdpFlowHandle, Error>
where
    S: AsyncSocket + tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
{
    let (stream, flow_io) = establish_udp_flow(stream, session, identity).await?;
    let initial_packet =
        VmessInitialUdpFlowPacket::from_parts(&session.target, session.port, initial_payload);
    let initial_packet_len = initial_packet.encoded_len(&flow_io)?;
    let handle = spawn_udp_flow(stream, Some(initial_packet), flow_io);

    Ok(VmessEstablishedUdpFlowHandle {
        handle,
        initial_packet_len,
    })
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
    pub fn response_mode(&self, state: VmessUdpPayloadState) -> VmessUdpPayloadMode {
        match state {
            VmessUdpPayloadState::Unknown
            | VmessUdpPayloadState::Mode(VmessUdpPayloadMode::VmessPacket) => {
                VmessUdpPayloadMode::VmessPacket
            }
            VmessUdpPayloadState::Mode(VmessUdpPayloadMode::RawDatagram) => {
                VmessUdpPayloadMode::RawDatagram
            }
        }
    }

    pub fn encode_response(
        &self,
        mode: VmessUdpPayloadMode,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_inbound_udp_response(mode, target, port, payload)
    }

    pub fn encode_response_for_state(
        &self,
        state: VmessUdpPayloadState,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.encode_response(self.response_mode(state), target, port, payload)
    }

    pub async fn write_response_tokio<W>(
        &self,
        writer: &mut W,
        state: VmessUdpPayloadState,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let packet = self.encode_response_for_state(state, target, port, payload)?;
        let len = packet.len();
        tokio::io::AsyncWriteExt::write_all(writer, &packet)
            .await
            .map_err(|_| Error::Io("failed to write VMess UDP response"))?;
        Ok(len)
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

    pub fn encode_mux_response_for_state(
        &self,
        mux_session_id: u16,
        state: VmessUdpPayloadState,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.encode_mux_response(
            mux_session_id,
            self.response_mode(state),
            target,
            port,
            payload,
        )
    }

    pub fn send_mux_response(
        &self,
        write_tx: &mpsc::UnboundedSender<Vec<u8>>,
        mux_session_id: u16,
        state: VmessUdpPayloadState,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        let frame =
            self.encode_mux_response_for_state(mux_session_id, state, target, port, payload)?;
        let len = frame.len();
        write_tx
            .send(frame)
            .map_err(|_| Error::Io("failed to queue VMess MUX UDP response"))?;
        Ok(len)
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
