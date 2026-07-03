use alloc::string::String;
#[cfg(feature = "reality")]
use alloc::vec;
use alloc::vec::Vec;

#[cfg(feature = "reality")]
use tokio::sync::{broadcast, mpsc, oneshot};
use zero_core::{Address, Error, InboundUdpDispatch, ProtocolType, Session};
#[cfg(feature = "reality")]
use zero_core::{MuxUdpDecodeFailure, MuxUdpResponder, StreamUdpResponder};
use zero_traits::{AsyncSocket, UdpPacketFraming, UdpPacketTunnelProtocol};

use crate::outbound::VlessOutbound;
use crate::shared::{
    parse_uuid, read_response, write_address, ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, CMD_UDP,
};

/// Target parameters for VLESS UDP packet tunnel over a connected stream.
#[derive(Debug, Clone, Copy)]
pub struct VlessUdpPacketTunnelTarget<'a> {
    pub session: &'a Session,
    pub id: &'a [u8; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlessUdpIdentity {
    pub uuid: [u8; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlessUdpMuxOpenIdentity {
    pub id: [u8; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VlessUdpFlowConfig<'a> {
    identity: VlessUdpIdentity,
    flow: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessUdpFlowResume {
    identity: VlessUdpIdentity,
    flow: Option<String>,
    relay_chain: bool,
}

impl VlessUdpFlowResume {
    pub fn identity(&self) -> VlessUdpIdentity {
        self.identity
    }

    pub fn mux_flow_enabled(&self) -> bool {
        matches!(
            self.flow.as_deref(),
            Some("xtls-rprx-vision") | Some("xtls-rprx-vision-udp443")
        )
    }

    #[cfg(feature = "reality")]
    pub fn mux_pool_identity(&self) -> crate::mux_pool::MuxIdentity {
        crate::mux_pool::MuxIdentity::from_uuid(self.identity.uuid)
    }

    pub fn flow_requires_relay_upstream(&self) -> bool {
        self.relay_chain
    }

    pub fn connector_flow(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> VlessUdpConnectorFlow {
        VlessUdpConnectorFlow {
            cache_key: format!(
                "vless:{server}:{port}:{session_id}:relay={}",
                self.relay_chain
            ),
            requires_relay_upstream: self.flow_requires_relay_upstream(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VlessUdpConnectorFlow {
    cache_key: String,
    requires_relay_upstream: bool,
}

impl VlessUdpConnectorFlow {
    pub fn into_parts(self) -> (String, bool) {
        (self.cache_key, self.requires_relay_upstream)
    }
}

impl<'a> VlessUdpFlowConfig<'a> {
    pub fn new(id: &str, flow: Option<&'a str>) -> Result<Self, Error> {
        Ok(Self {
            identity: parse_udp_identity(id)?,
            flow,
        })
    }

    pub fn identity(&self) -> VlessUdpIdentity {
        self.identity
    }

    pub fn uuid(&self) -> &[u8; 16] {
        &self.identity.uuid
    }

    pub fn mux_flow_enabled(&self) -> bool {
        self.flow == Some("xtls-rprx-vision") || self.flow == Some("xtls-rprx-vision-udp443")
    }

    pub fn mux_open_identity(&self) -> VlessUdpMuxOpenIdentity {
        VlessUdpMuxOpenIdentity {
            id: self.identity.uuid,
        }
    }

    pub fn flow_resume(&self, relay_chain: bool) -> VlessUdpFlowResume {
        VlessUdpFlowResume {
            identity: self.identity,
            flow: self.flow.map(Into::into),
            relay_chain,
        }
    }

    #[cfg(feature = "reality")]
    pub fn mux_pool_identity(&self) -> crate::mux_pool::MuxIdentity {
        crate::mux_pool::MuxIdentity::from_uuid(self.identity.uuid)
    }

    #[cfg(feature = "reality")]
    pub fn encode_initial_flow_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_flow_initial_packet(target, port, payload)
    }

    #[cfg(feature = "reality")]
    pub fn mux_initial_flow_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<VlessMuxInitialUdpFlowPacket, Error> {
        Ok(VlessMuxInitialUdpFlowPacket {
            packet: self.encode_initial_flow_packet(target, port, payload)?,
        })
    }

    #[cfg(feature = "reality")]
    pub async fn establish_flow_with_initial_packet<S>(
        &self,
        stream: S,
        session: &Session,
        initial_payload: &[u8],
    ) -> Result<VlessEstablishedUdpFlowHandle, Error>
    where
        S: AsyncSocket + tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
    {
        establish_udp_flow_with_initial_packet(stream, session, self.identity, initial_payload)
            .await
    }
}

pub fn udp_flow_config_from_config<'a>(
    id: &str,
    flow: Option<&'a str>,
) -> Result<VlessUdpFlowConfig<'a>, Error> {
    VlessUdpFlowConfig::new(id, flow)
}

pub fn udp_flow_resume_from_config(
    id: &str,
    flow: Option<&str>,
    relay_chain: bool,
) -> Result<VlessUdpFlowResume, Error> {
    VlessUdpFlowConfig::new(id, flow).map(|config| config.flow_resume(relay_chain))
}

pub fn connector_flow_from_resume(
    resume: &VlessUdpFlowResume,
    server: &str,
    port: u16,
    session_id: u64,
) -> VlessUdpConnectorFlow {
    resume.connector_flow(server, port, session_id)
}

pub fn parse_udp_identity(id: &str) -> Result<VlessUdpIdentity, Error> {
    parse_uuid(id).map(|uuid| VlessUdpIdentity { uuid })
}

pub async fn establish_udp_flow_stream<S>(
    stream: &mut S,
    session: &Session,
    identity: VlessUdpIdentity,
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    establish_udp_packet_tunnel(stream, session, &identity.uuid).await
}

#[cfg(feature = "reality")]
#[derive(Debug, Clone, Copy, Default)]
pub struct VlessEstablishedUdpFlow {
    io: VlessUdpFlowIo,
}

#[cfg(feature = "reality")]
pub type VlessUdpFlowResponse = (Address, u16, Vec<u8>);

#[cfg(feature = "reality")]
type VlessUdpFlowResponses = broadcast::Sender<VlessUdpFlowResponse>;

#[cfg(feature = "reality")]
pub type VlessUdpFlowResponseReceiver = broadcast::Receiver<VlessUdpFlowResponse>;

#[cfg(feature = "reality")]
struct VlessUdpFlowSend {
    packet: zero_core::UdpFlowPacket,
    result_tx: oneshot::Sender<Result<usize, Error>>,
}

#[cfg(feature = "reality")]
#[derive(Clone)]
pub struct VlessInitialUdpFlowPacket {
    packet: zero_core::UdpFlowPacket,
}

#[cfg(feature = "reality")]
impl VlessInitialUdpFlowPacket {
    pub fn from_parts(target: &Address, port: u16, payload: &[u8]) -> Self {
        Self {
            packet: zero_core::UdpFlowPacket::from_parts(target, port, payload),
        }
    }

    pub fn encoded_len(&self, flow: &VlessEstablishedUdpFlow) -> Result<usize, Error> {
        flow.encoded_packet_len(&self.packet.target, self.packet.port, &self.packet.payload)
    }

    pub fn encode(&self, flow: &VlessEstablishedUdpFlow) -> Result<Vec<u8>, Error> {
        flow.initial_packet(&self.packet.target, self.packet.port, &self.packet.payload)
    }

    fn write_target(&self) -> (&Address, u16, &[u8]) {
        (&self.packet.target, self.packet.port, &self.packet.payload)
    }
}

#[cfg(feature = "reality")]
#[derive(Clone)]
struct VlessUdpFlowSender {
    send_tx: mpsc::Sender<VlessUdpFlowSend>,
}

#[cfg(feature = "reality")]
pub struct VlessUdpFlowHandle {
    sender: VlessUdpFlowSender,
    responses: VlessUdpFlowResponses,
}

#[cfg(feature = "reality")]
pub struct VlessEstablishedUdpFlowHandle {
    pub handle: VlessUdpFlowHandle,
    pub initial_packet_len: usize,
}

#[cfg(feature = "reality")]
pub struct VlessMuxInitialUdpFlowPacket {
    packet: Vec<u8>,
}

#[cfg(feature = "reality")]
impl VlessMuxInitialUdpFlowPacket {
    pub fn encoded_len(&self) -> usize {
        self.packet.len()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.packet
    }
}

#[cfg(feature = "reality")]
impl VlessEstablishedUdpFlowHandle {
    pub fn into_connection(self) -> VlessUdpFlowConnection {
        VlessUdpFlowConnection::new(self.handle)
    }
}

#[cfg(feature = "reality")]
#[derive(Clone)]
pub struct VlessUdpFlowSession {
    sender: VlessUdpFlowSender,
    responses: VlessUdpFlowResponses,
}

#[cfg(feature = "reality")]
impl VlessUdpFlowSession {
    pub fn new(handle: VlessUdpFlowHandle) -> Self {
        Self {
            sender: handle.sender,
            responses: handle.responses,
        }
    }

    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        self.sender.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> VlessUdpFlowResponseReceiver {
        self.responses.subscribe()
    }
}

#[cfg(feature = "reality")]
#[derive(Clone)]
pub struct VlessUdpFlowConnection {
    session: VlessUdpFlowSession,
}

#[cfg(feature = "reality")]
impl VlessUdpFlowConnection {
    pub fn new(handle: VlessUdpFlowHandle) -> Self {
        Self {
            session: VlessUdpFlowSession::new(handle),
        }
    }

    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        self.session.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> VlessUdpFlowResponseReceiver {
        self.session.subscribe_responses()
    }
}

#[cfg(feature = "reality")]
impl VlessUdpFlowSender {
    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        let packet = zero_core::UdpFlowPacket::from_parts(target, port, payload);
        let (result_tx, result_rx) = oneshot::channel();
        self.send_tx
            .send(VlessUdpFlowSend { packet, result_tx })
            .await
            .map_err(|_| Error::Io("vless udp flow closed"))?;
        result_rx
            .await
            .map_err(|_| Error::Io("vless udp flow closed"))?
    }
}

#[cfg(feature = "reality")]
impl VlessEstablishedUdpFlow {
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
    ) -> Result<Option<VlessUdpFlowPacket>, Error>
    where
        S: tokio::io::AsyncRead + Unpin,
    {
        self.io.read_packet_tokio(stream, buffer).await
    }
}

#[cfg(feature = "reality")]
pub fn spawn_udp_flow<S>(
    stream: S,
    initial_packet: Option<VlessInitialUdpFlowPacket>,
    flow_io: VlessEstablishedUdpFlow,
) -> VlessUdpFlowHandle
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
{
    let (send_tx, send_rx) = mpsc::channel::<VlessUdpFlowSend>(32);
    let (responses, _) = broadcast::channel::<VlessUdpFlowResponse>(32);
    spawn_udp_flow_task(stream, initial_packet, send_rx, responses.clone(), flow_io);
    VlessUdpFlowHandle {
        sender: VlessUdpFlowSender { send_tx },
        responses,
    }
}

#[cfg(feature = "reality")]
pub fn start_mux_udp_flow(
    up_tx: mpsc::UnboundedSender<Vec<u8>>,
    down_rx: mpsc::UnboundedReceiver<Vec<u8>>,
) -> VlessUdpFlowConnection {
    let (send_tx, send_rx) = mpsc::channel::<VlessUdpFlowSend>(32);
    let (responses, _) = broadcast::channel::<VlessUdpFlowResponse>(32);
    spawn_mux_udp_flow_task(send_rx, up_tx, down_rx, responses.clone());
    VlessUdpFlowConnection::new(VlessUdpFlowHandle {
        sender: VlessUdpFlowSender { send_tx },
        responses,
    })
}

#[cfg(feature = "reality")]
pub async fn establish_udp_flow_with_initial_packet<S>(
    mut stream: S,
    session: &Session,
    identity: VlessUdpIdentity,
    initial_payload: &[u8],
) -> Result<VlessEstablishedUdpFlowHandle, Error>
where
    S: AsyncSocket + tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
{
    let flow_io = establish_udp_flow(&mut stream, session, identity).await?;
    let initial_packet =
        VlessInitialUdpFlowPacket::from_parts(&session.target, session.port, initial_payload);
    let initial_packet_len = initial_packet.encoded_len(&flow_io)?;
    let handle = spawn_udp_flow(stream, Some(initial_packet), flow_io);

    Ok(VlessEstablishedUdpFlowHandle {
        handle,
        initial_packet_len,
    })
}

#[cfg(feature = "reality")]
pub async fn establish_udp_flow_with_resume<S>(
    mut stream: S,
    session: &Session,
    resume: &VlessUdpFlowResume,
) -> Result<VlessUdpFlowConnection, Error>
where
    S: AsyncSocket + tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
{
    let flow_io = establish_udp_flow(&mut stream, session, resume.identity()).await?;
    Ok(VlessUdpFlowConnection::new(spawn_udp_flow(
        stream, None, flow_io,
    )))
}

#[cfg(feature = "reality")]
fn spawn_udp_flow_task<S>(
    mut stream: S,
    initial_packet: Option<VlessInitialUdpFlowPacket>,
    mut send_rx: mpsc::Receiver<VlessUdpFlowSend>,
    responses: VlessUdpFlowResponses,
    flow_io: VlessEstablishedUdpFlow,
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
                            let (target, port, payload) = request.packet.into_parts();
                            let result = flow_io
                                .write_packet_tokio(&mut stream, &target, port, &payload)
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

#[cfg(feature = "reality")]
fn spawn_mux_udp_flow_task(
    mut send_rx: mpsc::Receiver<VlessUdpFlowSend>,
    up_tx: mpsc::UnboundedSender<Vec<u8>>,
    mut down_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    responses: VlessUdpFlowResponses,
) {
    tokio::spawn(async move {
        let io = VlessUdpFlowIo;
        loop {
            tokio::select! {
                to_send = send_rx.recv() => {
                    match to_send {
                        Some(request) => {
                            let (target, port, payload) = request.packet.into_parts();
                            let result = io
                                .encode_packet(&target, port, &payload)
                                .and_then(|packet| {
                                    let len = packet.len();
                                    up_tx
                                        .send(packet)
                                        .map(|_| len)
                                        .map_err(|_| Error::Io("vless mux udp flow closed"))
                                });
                            let should_break = result.is_err();
                            let _ = request.result_tx.send(result);
                            if should_break {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                read = down_rx.recv() => {
                    match read {
                        Some(packet) => match io.decode_packet(&packet) {
                            Ok(packet) => {
                                let _ = responses.send(packet.into_parts());
                            }
                            Err(_) => break,
                        },
                        None => break,
                    }
                }
            }
        }
    });
}

#[cfg(feature = "reality")]
pub async fn establish_udp_flow<S>(
    stream: &mut S,
    session: &Session,
    identity: VlessUdpIdentity,
) -> Result<VlessEstablishedUdpFlow, Error>
where
    S: AsyncSocket,
{
    establish_udp_flow_stream(stream, session, identity).await?;
    Ok(VlessEstablishedUdpFlow { io: VlessUdpFlowIo })
}

impl<'a> UdpPacketTunnelProtocol<VlessUdpPacketTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn establish_udp_packet_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VlessUdpPacketTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        establish_udp_packet_tunnel(stream, target.session, target.id).await
    }
}

fn build_udp_request(session: &Session, id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    crate::shared::build_request(session, id, CMD_UDP)
}

pub async fn send_udp_request<S>(
    stream: &mut S,
    session: &Session,
    id: &[u8; 16],
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    if session.port == 0 {
        return Err(Error::Config("target port is required"));
    }

    let request = build_udp_request(session, id)?;
    stream
        .write_all(&request)
        .await
        .map_err(|_| Error::Io("failed to write VLESS UDP request"))
}

pub async fn establish_udp_packet_tunnel<S>(
    stream: &mut S,
    session: &Session,
    id: &[u8; 16],
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    send_udp_request(stream, session, id).await?;
    read_response(stream).await
}

/// One UDP datagram to encode for a VLESS UDP packet tunnel.
#[derive(Debug, Clone, Copy)]
pub struct VlessUdpPacketTarget<'a> {
    pub address: &'a Address,
    pub port: u16,
    pub payload: &'a [u8],
}

impl<'a> UdpPacketFraming<VlessUdpPacketTarget<'a>> for VlessOutbound {
    type Error = Error;
    type Decoded = VlessUdpPacket;

    fn encode_udp_packet(&self, packet: &VlessUdpPacketTarget<'a>) -> Result<Vec<u8>, Self::Error> {
        build_udp_packet(packet.address, packet.port, packet.payload)
    }

    fn decode_udp_packet(&self, packet: &[u8]) -> Result<Self::Decoded, Self::Error> {
        parse_udp_packet(packet)
    }
}

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
