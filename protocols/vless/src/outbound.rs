use alloc::vec::Vec;

#[cfg(feature = "reality")]
use tokio::sync::{broadcast, mpsc, oneshot};
use zero_core::{Error, ProtocolType, Session};
#[cfg(feature = "reality")]
use zero_traits::DeferredTcpTunnelProtocol;
use zero_traits::{AsyncSocket, TcpTunnelProtocol, UdpPacketFraming, UdpPacketTunnelProtocol};

#[cfg(feature = "reality")]
use crate::flow::flow_build_request;
use crate::mux::MuxClient;
use crate::shared::{
    build_udp_packet, parse_udp_packet, read_addon, read_exact, write_address, CMD_MUX,
    VLESS_VERSION,
};
#[cfg(feature = "reality")]
use crate::VlessUdpFlowIo;
use crate::VlessUdpPacket;

#[derive(Debug, Default, Clone, Copy)]
pub struct VlessOutbound;

impl VlessOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Vless
    }

    pub async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_tcp_request(stream, session, id).await?;
        read_response(stream).await
    }

    #[cfg(feature = "reality")]
    pub async fn establish_tcp_tunnel_with_flow<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
        flow: Option<&str>,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_tcp_request_with_flow(stream, session, id, flow)
            .await?;
        read_response(stream).await
    }

    pub async fn send_tcp_request<S>(
        &self,
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

        let request = build_tcp_request(session, id)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS outbound request"))
    }

    #[cfg(feature = "reality")]
    pub async fn send_tcp_request_with_flow<S>(
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
        flow: Option<&str>,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        if session.port == 0 {
            return Err(Error::Config("target port is required"));
        }

        let request = build_tcp_request_with_flow(session, id, flow)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS outbound request"))
    }

    pub async fn send_udp_request<S>(
        &self,
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
        &self,
        stream: &mut S,
        session: &Session,
        id: &[u8; 16],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_udp_request(stream, session, id).await?;
        read_response(stream).await
    }

    /// Send VLESS MUX header and read server response.
    /// Returns a MuxClient for subsequent stream allocation.
    pub async fn establish_mux<S>(&self, stream: &mut S, id: &[u8; 16]) -> Result<MuxClient, Error>
    where
        S: AsyncSocket,
    {
        let request = build_mux_request(id)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("failed to write VLESS MUX request"))?;
        read_response(stream).await?;
        Ok(MuxClient::new())
    }
}

/// Target parameters for VLESS TCP tunnel (non-flow path).
#[derive(Debug, Clone, Copy)]
pub struct VlessTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub id: &'a [u8; 16],
}

impl<'a> TcpTunnelProtocol<VlessTcpTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VlessTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.establish_tcp_tunnel(stream, target.session, target.id)
            .await
    }
}

/// Target parameters for VLESS TCP tunnel with flow (Vision/Reality path).
///
/// The flow parameter controls the XTLS Vision flow negotiation. When `None`,
/// the handshake uses the standard path. This target is only available when
/// the `reality` feature is enabled because flow handling requires the
/// Vision/Reality code path.
#[cfg(feature = "reality")]
#[derive(Debug, Clone, Copy)]
pub struct VlessFlowTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub id: &'a [u8; 16],
    pub flow: Option<&'a str>,
}

#[cfg(feature = "reality")]
impl<'a> TcpTunnelProtocol<VlessFlowTcpTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &VlessFlowTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        match target.flow {
            Some(f) => {
                self.establish_tcp_tunnel_with_flow(stream, target.session, target.id, Some(f))
                    .await
            }
            None => {
                self.establish_tcp_tunnel(stream, target.session, target.id)
                    .await
            }
        }
    }
}

#[cfg(feature = "reality")]
impl<'a> DeferredTcpTunnelProtocol<VlessFlowTcpTunnelTarget<'a>> for VlessOutbound {
    type Error = Error;

    async fn send_deferred_tcp_tunnel_request<S>(
        &self,
        stream: &mut S,
        target: &VlessFlowTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.send_tcp_request_with_flow(stream, target.session, target.id, target.flow)
            .await
    }
}

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

pub fn parse_udp_identity(id: &str) -> Result<VlessUdpIdentity, Error> {
    crate::shared::parse_uuid(id).map(|uuid| VlessUdpIdentity { uuid })
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
pub type VlessUdpFlowResponse = (zero_core::Address, u16, Vec<u8>);

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
    pub fn from_parts(target: &zero_core::Address, port: u16, payload: &[u8]) -> Self {
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

    fn write_target(&self) -> (&zero_core::Address, u16, &[u8]) {
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

    pub async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.sender.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> VlessUdpFlowResponseReceiver {
        self.responses.subscribe()
    }
}

#[cfg(feature = "reality")]
impl VlessUdpFlowSender {
    pub async fn send(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
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
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.io.encode_packet(target, port, payload)
    }

    pub fn encoded_packet_len(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.io.encoded_packet_len(target, port, payload)
    }

    pub fn initial_packet(
        &self,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.io.encode_packet(target, port, payload)
    }

    pub async fn write_packet_tokio<S>(
        &self,
        stream: &mut S,
        target: &zero_core::Address,
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
    ) -> Result<Option<crate::VlessUdpFlowPacket>, Error>
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

        let mut buffer = alloc::vec![0_u8; 64 * 1024];
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
        self.establish_udp_packet_tunnel(stream, target.session, target.id)
            .await
    }
}

pub async fn establish_udp_packet_tunnel<S>(
    stream: &mut S,
    session: &Session,
    id: &[u8; 16],
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    VlessOutbound
        .establish_udp_packet_tunnel(stream, session, id)
        .await
}

/// One UDP datagram to encode for a VLESS UDP packet tunnel.
#[derive(Debug, Clone, Copy)]
pub struct VlessUdpPacketTarget<'a> {
    pub address: &'a zero_core::Address,
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

fn build_tcp_request(session: &Session, id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(crate::shared::CMD_TCP);
    request.extend_from_slice(&session.port.to_be_bytes());
    write_address(&mut request, &session.target)?;

    Ok(request)
}

fn build_udp_request(session: &Session, id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(crate::shared::CMD_UDP);
    request.extend_from_slice(&session.port.to_be_bytes());
    write_address(&mut request, &session.target)?;

    Ok(request)
}

fn build_mux_request(id: &[u8; 16]) -> Result<Vec<u8>, Error> {
    let mut request = Vec::with_capacity(24);
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(0x00);
    request.push(CMD_MUX);
    // Dummy target — ignored by the MUX server
    request.extend_from_slice(&0u16.to_be_bytes());
    request.push(0x01); // ATYP_IPV4
    request.extend_from_slice(&[0u8; 4]);

    Ok(request)
}

#[cfg(feature = "reality")]
fn build_tcp_request_with_flow(
    session: &Session,
    id: &[u8; 16],
    flow: Option<&str>,
) -> Result<Vec<u8>, Error> {
    let (fbyte, payload) = flow_build_request(
        id,
        flow,
        crate::shared::CMD_TCP,
        session.port,
        &session.target,
    )?;

    let mut request = Vec::with_capacity(24 + payload.len());
    request.push(VLESS_VERSION);
    request.extend_from_slice(id);
    request.push(fbyte);
    request.extend_from_slice(&payload);

    Ok(request)
}

async fn read_response<S>(stream: &mut S) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let mut version = [0_u8; 1];
    read_exact(stream, &mut version).await?;
    if version[0] != VLESS_VERSION {
        return Err(Error::Protocol("unsupported VLESS response version"));
    }

    read_addon(stream).await
}
