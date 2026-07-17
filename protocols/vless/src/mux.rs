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

use alloc::boxed::Box;
use alloc::vec::Vec;

#[cfg(feature = "reality")]
use tokio::sync::mpsc;
use zero_core::{Address, Error, Network, ProtocolType, Session};
#[cfg(feature = "reality")]
use zero_core::{
    InboundMuxTcpRelay, InboundMuxUdpReadFailure, InboundMuxUdpReadFailureAction,
    InboundMuxUdpRelay, MuxUdpDecodeFailure, MuxUdpResponder, SessionAuth,
};
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
pub(crate) struct MuxFrame {
    pub session_id: u16,
    pub status: u8,
    pub options: u8,
    pub payload: Vec<u8>,
}

/// Target info for a new MUX stream.
#[derive(Debug, Clone)]
struct MuxTarget {
    pub network: u8,
    pub port: u16,
    pub address: Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MuxNetwork {
    Tcp,
    Udp,
}

impl MuxTarget {
    fn network_kind(&self) -> Result<MuxNetwork, Error> {
        match self.network {
            NETWORK_TCP => Ok(MuxNetwork::Tcp),
            NETWORK_UDP => Ok(MuxNetwork::Udp),
            _ => Err(Error::Protocol("MUX new stream unknown network type")),
        }
    }

    fn into_session(self) -> Result<Session, Error> {
        let network = match self.network_kind()? {
            MuxNetwork::Tcp => Network::Tcp,
            MuxNetwork::Udp => Network::Udp,
        };
        Ok(Session::new(
            0,
            self.address,
            self.port,
            network,
            ProtocolType::new("vless"),
        ))
    }
}

#[derive(Debug, Clone)]
enum MuxServerEvent {
    KeepAlive,
    NewStream { session_id: u16, target: MuxTarget },
    Data { session_id: u16, payload: Vec<u8> },
    End { session_id: u16 },
    Unknown { session_id: u16 },
}

#[derive(Debug, Clone)]
enum VlessInboundMuxAction {
    KeepAlive,
    OpenStream {
        session_id: u16,
        session: Box<Session>,
    },
    Data {
        session_id: u16,
        payload: Vec<u8>,
    },
    End {
        session_id: u16,
    },
    Unknown {
        session_id: u16,
    },
}

#[cfg(feature = "reality")]
struct VlessInboundMuxOpenedStream {
    session_id: u16,
    session: Box<Session>,
    up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

#[cfg(feature = "reality")]
enum VlessInboundMuxOpenedRouteState {
    Tcp {
        session: Session,
        relay: VlessInboundMuxTcpRelay,
    },
    Udp {
        relay: VlessInboundMuxUdpRelay,
    },
}

#[cfg(feature = "reality")]
struct VlessInboundMuxOpenedRoute {
    state: VlessInboundMuxOpenedRouteState,
}

#[cfg(feature = "reality")]
impl VlessInboundMuxOpenedRoute {
    fn tcp(session: Session, relay: VlessInboundMuxTcpRelay) -> Self {
        Self {
            state: VlessInboundMuxOpenedRouteState::Tcp { session, relay },
        }
    }

    fn udp(relay: VlessInboundMuxUdpRelay) -> Self {
        Self {
            state: VlessInboundMuxOpenedRouteState::Udp { relay },
        }
    }
}

#[cfg(feature = "reality")]
pub struct VlessInboundMuxTcpRelay {
    session_id: u16,
    up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    writer: VlessInboundMuxWriter,
}

#[cfg(feature = "reality")]
impl VlessInboundMuxTcpRelay {
    fn new(
        session_id: u16,
        up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        writer: VlessInboundMuxWriter,
    ) -> Self {
        Self {
            session_id,
            up_rx,
            writer,
        }
    }

    async fn relay_stream<S>(self, upstream: S)
    where
        S: AsyncSocket + 'static,
        S::Error: Send,
    {
        relay_inbound_mux_stream(self.session_id, self.up_rx, self.writer, upstream).await;
    }
}

#[cfg(feature = "reality")]
impl InboundMuxTcpRelay for VlessInboundMuxTcpRelay {
    fn mux_session_id(&self) -> u16 {
        self.session_id
    }

    fn close_stream(&self) -> impl core::future::Future<Output = ()> + Send {
        let session_id = self.session_id;
        let writer = self.writer.clone();
        async move {
            let _ = writer.end_inbound_stream(session_id);
        }
    }

    async fn relay_stream<S>(self, upstream: S)
    where
        S: AsyncSocket + 'static,
        S::Error: Send,
    {
        VlessInboundMuxTcpRelay::relay_stream(self, upstream).await;
    }
}

#[cfg(feature = "reality")]
impl VlessInboundMuxOpenedStream {
    fn new(
        session_id: u16,
        session: Box<Session>,
        up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Self {
        Self {
            session_id,
            session,
            up_rx,
        }
    }

    fn into_parts(self) -> (u16, Session, mpsc::UnboundedReceiver<Vec<u8>>) {
        (self.session_id, *self.session, self.up_rx)
    }

    fn into_route_with_auth(
        self,
        auth: Option<&SessionAuth>,
        writer: VlessInboundMuxWriter,
    ) -> VlessInboundMuxOpenedRoute {
        let (session_id, mut session, up_rx) = self.into_parts();
        if let Some(auth) = auth {
            session.apply_auth(auth.clone());
        }
        match session.network {
            Network::Tcp => VlessInboundMuxOpenedRoute::tcp(
                session,
                VlessInboundMuxTcpRelay::new(session_id, up_rx, writer),
            ),
            Network::Udp => VlessInboundMuxOpenedRoute::udp(VlessInboundMuxUdpRelay::new(
                session_id,
                up_rx,
                crate::udp::VlessInboundMuxUdpResponder::new(
                    crate::udp::VlessInboundUdpSession::new(),
                    writer,
                    session_id,
                ),
                auth.cloned(),
            )),
        }
    }
}

#[cfg(feature = "reality")]
pub struct VlessInboundMuxUdpRelay {
    session_id: u16,
    up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    responder: crate::udp::VlessInboundMuxUdpResponder,
    auth: Option<SessionAuth>,
}

#[cfg(feature = "reality")]
impl VlessInboundMuxUdpRelay {
    fn new(
        session_id: u16,
        up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        responder: crate::udp::VlessInboundMuxUdpResponder,
        auth: Option<SessionAuth>,
    ) -> Self {
        Self {
            session_id,
            up_rx,
            responder,
            auth,
        }
    }
}

#[cfg(feature = "reality")]
#[async_trait::async_trait]
impl InboundMuxUdpRelay for VlessInboundMuxUdpRelay {
    async fn read_inbound_dispatch(
        &mut self,
    ) -> Result<Option<zero_core::InboundUdpDispatch>, InboundMuxUdpReadFailure> {
        let Some(payload) = self.up_rx.recv().await else {
            return Ok(None);
        };
        if payload.is_empty() {
            return Ok(None);
        }

        match self.responder.decode_inbound_dispatch(&payload) {
            Ok(inbound_dispatch) => Ok(Some(inbound_dispatch)),
            Err(error) => Err(InboundMuxUdpReadFailure {
                error,
                action: match self.responder.decode_failure() {
                    MuxUdpDecodeFailure::Continue => InboundMuxUdpReadFailureAction::Continue,
                    MuxUdpDecodeFailure::End => InboundMuxUdpReadFailureAction::End,
                },
            }),
        }
    }

    fn write_response_for_target(
        &mut self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.responder
            .write_response_for_target(target, port, payload)
    }

    fn end_inbound_stream(&mut self) -> Result<usize, Error> {
        self.responder.end_inbound_stream()
    }

    fn mux_session_id(&self) -> u16 {
        self.session_id
    }

    fn auth(&self) -> Option<&SessionAuth> {
        self.auth.as_ref()
    }
}

#[cfg(feature = "reality")]
#[derive(Clone)]
pub(crate) struct VlessInboundMuxWriter {
    down_tx: mpsc::UnboundedSender<VlessInboundMuxDownlink>,
}

#[cfg(feature = "reality")]
#[derive(Default)]
struct VlessInboundMuxStreams {
    streams: alloc::collections::BTreeMap<u16, mpsc::UnboundedSender<Vec<u8>>>,
}

#[cfg(feature = "reality")]
struct VlessInboundMuxDownlink {
    session_id: u16,
    payload: Vec<u8>,
}

#[cfg(feature = "reality")]
pub struct VlessInboundMuxServer {
    mux: VlessInboundMuxSession,
    streams: VlessInboundMuxStreams,
    writer: VlessInboundMuxWriter,
    down_rx: mpsc::UnboundedReceiver<VlessInboundMuxDownlink>,
    auth: Option<SessionAuth>,
}

#[cfg(feature = "reality")]
impl VlessInboundMuxServer {
    fn new(mux: VlessInboundMuxSession) -> Self {
        let (writer, down_rx) = VlessInboundMuxWriter::channel();
        Self {
            mux,
            streams: VlessInboundMuxStreams::new(),
            writer,
            down_rx,
            auth: None,
        }
    }

    pub(crate) fn from_master_uuid_with_auth(
        master_uuid: [u8; 16],
        auth: Option<SessionAuth>,
    ) -> Self {
        Self::new(VlessInboundMuxSession::with_encryption(&master_uuid)).with_auth(auth)
    }

    fn with_auth(mut self, auth: Option<SessionAuth>) -> Self {
        self.auth = auth;
        self
    }

    fn writer(&self) -> VlessInboundMuxWriter {
        self.writer.clone()
    }

    async fn next_opened_route_with_auth<S>(
        &mut self,
        stream: &mut S,
        auth: Option<&SessionAuth>,
    ) -> Result<Option<VlessInboundMuxOpenedRoute>, Error>
    where
        S: AsyncSocket,
    {
        let opened = loop {
            tokio::select! {
                action = self.mux.read_inbound_action(stream) => {
                    let opened = self
                        .streams
                        .apply_inbound_action(&mut self.mux, stream, action?)
                        .await?;
                    break opened;
                }
                downlink = self.down_rx.recv() => {
                    let Some(downlink) = downlink else {
                        continue;
                    };
                    let _ = self
                        .streams
                        .send_inbound_downlink(&mut self.mux, stream, downlink)
                        .await?;
                }
            }
        };
        let writer = self.writer();
        Ok(opened.map(|opened| opened.into_route_with_auth(auth, writer)))
    }

    async fn next_opened_route<S>(
        &mut self,
        stream: &mut S,
    ) -> Result<Option<VlessInboundMuxOpenedRoute>, Error>
    where
        S: AsyncSocket,
    {
        let auth = self.auth.clone();
        self.next_opened_route_with_auth(stream, auth.as_ref())
            .await
    }
}

#[cfg(feature = "reality")]
#[async_trait::async_trait]
impl<S> zero_core::InboundMuxServer<S> for VlessInboundMuxServer
where
    S: AsyncSocket,
{
    type TcpRelay = VlessInboundMuxTcpRelay;
    type UdpRelay = VlessInboundMuxUdpRelay;

    async fn dispatch_next_opened_route<E, FTcp, FUdp>(
        &mut self,
        stream: &mut S,
        on_tcp_opened: FTcp,
        on_udp_opened: FUdp,
    ) -> Result<bool, E>
    where
        E: From<Error>,
        FTcp: FnOnce(Session, Self::TcpRelay) -> Result<(), E> + Send,
        FUdp: FnOnce(Self::UdpRelay) -> Result<(), E> + Send,
    {
        let Some(route) = self.next_opened_route(stream).await? else {
            return Ok(false);
        };

        match route.state {
            VlessInboundMuxOpenedRouteState::Tcp { session, relay } => {
                on_tcp_opened(session, relay)?;
            }
            VlessInboundMuxOpenedRouteState::Udp { relay } => {
                on_udp_opened(relay)?;
            }
        }

        Ok(true)
    }
}

#[cfg(feature = "reality")]
impl VlessInboundMuxStreams {
    fn new() -> Self {
        Self::default()
    }

    fn open_stream(&mut self, session_id: u16) -> mpsc::UnboundedReceiver<Vec<u8>> {
        let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.streams.insert(session_id, tx);
        rx
    }

    fn push_stream_data(&self, session_id: u16, payload: Vec<u8>) -> bool {
        self.streams
            .get(&session_id)
            .is_some_and(|tx| tx.send(payload).is_ok())
    }

    fn close_inbound_stream(&mut self, session_id: u16) -> bool {
        self.streams.remove(&session_id).is_some()
    }

    fn contains_stream(&self, session_id: u16) -> bool {
        self.streams.contains_key(&session_id)
    }

    async fn apply_inbound_action<S>(
        &mut self,
        mux: &mut VlessInboundMuxSession,
        stream: &mut S,
        action: VlessInboundMuxAction,
    ) -> Result<Option<VlessInboundMuxOpenedStream>, Error>
    where
        S: AsyncSocket,
    {
        match action {
            VlessInboundMuxAction::KeepAlive => Ok(None),
            VlessInboundMuxAction::OpenStream {
                session_id,
                session,
            } => {
                mux.accept_inbound_stream(stream, session_id).await?;
                let up_rx = self.open_stream(session_id);
                Ok(Some(VlessInboundMuxOpenedStream::new(
                    session_id, session, up_rx,
                )))
            }
            VlessInboundMuxAction::Data {
                session_id,
                payload,
            } => {
                if !self.push_stream_data(session_id, payload) {
                    mux.end_inbound_stream(stream, session_id).await?;
                }
                Ok(None)
            }
            VlessInboundMuxAction::End { session_id } => {
                self.close_inbound_stream(session_id);
                Ok(None)
            }
            VlessInboundMuxAction::Unknown { session_id } => {
                mux.reject_inbound_stream(stream).await?;
                self.close_inbound_stream(session_id);
                Ok(None)
            }
        }
    }

    async fn send_inbound_downlink<S>(
        &mut self,
        mux: &mut VlessInboundMuxSession,
        stream: &mut S,
        downlink: VlessInboundMuxDownlink,
    ) -> Result<bool, Error>
    where
        S: AsyncSocket,
    {
        let sid = downlink.session_id();
        if !self.contains_stream(sid) {
            return Ok(false);
        }

        let should_close = downlink.is_end();
        let (sid, payload) = downlink.into_parts();
        mux.send_inbound_stream_payload(stream, sid, &payload)
            .await?;
        if should_close {
            self.close_inbound_stream(sid);
        }
        Ok(true)
    }
}

#[cfg(feature = "reality")]
impl VlessInboundMuxDownlink {
    fn new(session_id: u16, payload: Vec<u8>) -> Self {
        Self {
            session_id,
            payload,
        }
    }

    fn session_id(&self) -> u16 {
        self.session_id
    }

    fn is_end(&self) -> bool {
        self.payload.is_empty()
    }

    fn into_parts(self) -> (u16, Vec<u8>) {
        (self.session_id, self.payload)
    }
}

#[cfg(feature = "reality")]
impl VlessInboundMuxWriter {
    fn new(down_tx: mpsc::UnboundedSender<VlessInboundMuxDownlink>) -> Self {
        Self { down_tx }
    }

    fn channel() -> (Self, mpsc::UnboundedReceiver<VlessInboundMuxDownlink>) {
        let (down_tx, down_rx) = mpsc::unbounded_channel::<VlessInboundMuxDownlink>();
        (Self::new(down_tx), down_rx)
    }

    pub(crate) fn data(&self, session_id: u16, payload: Vec<u8>) -> Result<usize, Error> {
        let len = payload.len();
        self.down_tx
            .send(VlessInboundMuxDownlink::new(session_id, payload))
            .map_err(|_| Error::Io("failed to queue VLESS MUX data"))?;
        Ok(len)
    }

    pub(crate) fn end(&self, session_id: u16) -> Result<usize, Error> {
        self.down_tx
            .send(VlessInboundMuxDownlink::new(session_id, Vec::new()))
            .map_err(|_| Error::Io("failed to queue VLESS MUX end"))?;
        Ok(0)
    }

    pub(crate) fn end_inbound_stream(&self, session_id: u16) -> Result<usize, Error> {
        self.end(session_id)
    }

    pub(crate) fn write_inbound_stream_payload(
        &self,
        session_id: u16,
        payload: Vec<u8>,
    ) -> Result<usize, Error> {
        if payload.is_empty() {
            self.end_inbound_stream(session_id)
        } else {
            self.data(session_id, payload)
        }
    }

    pub(crate) fn frame(&self, session_id: u16, frame: Vec<u8>) -> Result<usize, Error> {
        let len = frame.len();
        self.down_tx
            .send(VlessInboundMuxDownlink::new(session_id, frame))
            .map_err(|_| Error::Io("failed to queue VLESS MUX frame"))?;
        Ok(len)
    }
}

// ── frame encode / decode ──

#[cfg(feature = "reality")]
async fn relay_inbound_mux_stream<S>(
    session_id: u16,
    mut up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    writer: VlessInboundMuxWriter,
    mut upstream: S,
) where
    S: AsyncSocket + 'static,
    S::Error: Send,
{
    let mut upload_open = true;
    let mut buf = [0_u8; MUX_MAX_PAYLOAD];

    loop {
        tokio::select! {
            inbound = up_rx.recv(), if upload_open => {
                match inbound {
                    Some(data) => {
                        if upstream.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    None => {
                        upload_open = false;
                        let _ = upstream.shutdown().await;
                    }
                }
            }
            read = upstream.read(&mut buf) => {
                match read {
                    Ok(0) => break,
                    Ok(n) => {
                        if writer
                            .write_inbound_stream_payload(session_id, buf[..n].to_vec())
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }

    let _ = writer.write_inbound_stream_payload(session_id, Vec::new());
}

/// Encode a MUX frame: [length:2(BE)][session_id:2(BE)][status:1][options:1][payload…]
/// length covers session_id(2) + status(1) + options(1) + payload.
fn encode_frame(session_id: u16, status: u8, options: u8, payload: &[u8]) -> Vec<u8> {
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
async fn read_mux_frame<S>(stream: &mut S) -> Result<MuxFrame, Error>
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
pub(crate) fn encode_new_stream(
    network: u8,
    port: u16,
    address: &Address,
) -> Result<Vec<u8>, Error> {
    let mut payload = Vec::with_capacity(24);
    payload.push(network);
    payload.extend_from_slice(&port.to_be_bytes());
    write_address(&mut payload, address)?;
    Ok(encode_frame(MUX_STREAM_NEW, STATUS_NEW, 0, &payload))
}

/// Parse a new-stream payload into target info.
fn parse_new_stream(payload: &[u8]) -> Result<MuxTarget, Error> {
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
fn encode_new_stream_response(assigned_id: u16, status: u8) -> Vec<u8> {
    let mut payload = Vec::with_capacity(3);
    payload.extend_from_slice(&assigned_id.to_be_bytes());
    payload.push(status);
    encode_frame(MUX_STREAM_NEW, STATUS_NEW, 0, &payload)
}

/// Parse a new-stream response payload → (assigned_id, status).
// ── Data / End / KeepAlive frame helpers ──

/// Build a TCP data frame (STATUS_KEEP | OPTION_DATA).
pub(crate) fn encode_data_frame(session_id: u16, data: &[u8]) -> Vec<u8> {
    encode_frame(session_id, STATUS_KEEP, OPTION_DATA, data)
}

/// Format: [network:1][port:2][atyp:1][address…][data…]
/// Build an END frame (terminate the session).
pub(crate) fn encode_end_frame(session_id: u16) -> Vec<u8> {
    encode_frame(session_id, STATUS_END, 0, &[])
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

/// Minimal MUX client — manages stream allocation and frame I/O.
// ── mux server ─────────────────────────────────────────

/// MUX server-side handler — reads frames and dispatches.
struct MuxServer {
    next_id: u16,
    #[cfg(feature = "reality")]
    crypto: Option<crate::mux_crypto::MuxCrypto>,
}

struct VlessInboundMuxSession {
    server: MuxServer,
}

impl Default for VlessInboundMuxSession {
    fn default() -> Self {
        Self::new()
    }
}

impl VlessInboundMuxSession {
    fn new() -> Self {
        Self {
            server: MuxServer::new(),
        }
    }

    #[cfg(feature = "reality")]
    fn with_encryption(master_uuid: &[u8; 16]) -> Self {
        Self {
            server: MuxServer::with_encryption(master_uuid),
        }
    }

    async fn next_event<S>(&mut self, stream: &mut S) -> Result<MuxServerEvent, Error>
    where
        S: AsyncSocket,
    {
        self.server.recv_event(stream).await
    }

    async fn next_action<S>(&mut self, stream: &mut S) -> Result<VlessInboundMuxAction, Error>
    where
        S: AsyncSocket,
    {
        self.next_event(stream).await.map(Into::into)
    }

    async fn read_inbound_action<S>(
        &mut self,
        stream: &mut S,
    ) -> Result<VlessInboundMuxAction, Error>
    where
        S: AsyncSocket,
    {
        self.next_action(stream).await
    }

    async fn accept_stream<S>(&mut self, stream: &mut S, sid: u16) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.server.write_new_stream_accepted(stream, sid).await
    }

    async fn accept_inbound_stream<S>(&mut self, stream: &mut S, sid: u16) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.accept_stream(stream, sid).await
    }

    async fn reject_stream<S>(&mut self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.server.write_new_stream_rejected(stream).await
    }

    async fn reject_inbound_stream<S>(&mut self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.reject_stream(stream).await
    }

    async fn send_data<S>(&mut self, stream: &mut S, sid: u16, payload: &[u8]) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.server.write_data(stream, sid, payload).await
    }

    async fn send_inbound_stream_data<S>(
        &mut self,
        stream: &mut S,
        sid: u16,
        payload: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_data(stream, sid, payload).await
    }

    async fn send_inbound_stream_payload<S>(
        &mut self,
        stream: &mut S,
        sid: u16,
        payload: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        if payload.is_empty() {
            self.end_inbound_stream(stream, sid).await
        } else {
            self.send_inbound_stream_data(stream, sid, payload).await
        }
    }

    async fn end_stream<S>(&mut self, stream: &mut S, sid: u16) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.server.write_end(stream, sid).await
    }

    async fn end_inbound_stream<S>(&mut self, stream: &mut S, sid: u16) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.end_stream(stream, sid).await
    }
}

impl From<MuxServerEvent> for VlessInboundMuxAction {
    fn from(event: MuxServerEvent) -> Self {
        match event {
            MuxServerEvent::KeepAlive => Self::KeepAlive,
            MuxServerEvent::NewStream { session_id, target } => match target.into_session() {
                Ok(session) => Self::OpenStream {
                    session_id,
                    session: Box::new(session),
                },
                Err(_) => Self::Unknown { session_id },
            },
            MuxServerEvent::Data {
                session_id,
                payload,
            } => Self::Data {
                session_id,
                payload,
            },
            MuxServerEvent::End { session_id } => Self::End { session_id },
            MuxServerEvent::Unknown { session_id } => Self::Unknown { session_id },
        }
    }
}

impl Default for MuxServer {
    fn default() -> Self {
        Self::new()
    }
}

impl MuxServer {
    fn new() -> Self {
        Self {
            next_id: 1,
            #[cfg(feature = "reality")]
            crypto: None,
        }
    }

    #[cfg(feature = "reality")]
    fn with_encryption(master_uuid: &[u8; 16]) -> Self {
        Self {
            next_id: 1,
            crypto: Some(crate::mux_crypto::MuxCrypto::new(master_uuid)),
        }
    }

    fn alloc_id(&mut self) -> u16 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.next_id == 0 {
            self.next_id = 1;
        }
        id
    }

    async fn recv_event<S>(&mut self, stream: &mut S) -> Result<MuxServerEvent, Error>
    where
        S: AsyncSocket,
    {
        let frame = self.recv(stream).await?;
        match frame.status {
            STATUS_KEEP_ALIVE => Ok(MuxServerEvent::KeepAlive),
            STATUS_NEW => {
                let target = parse_new_stream(&frame.payload)?;
                let session_id = self.alloc_id();
                Ok(MuxServerEvent::NewStream { session_id, target })
            }
            STATUS_KEEP => Ok(MuxServerEvent::Data {
                session_id: frame.session_id,
                payload: frame.payload,
            }),
            STATUS_END => Ok(MuxServerEvent::End {
                session_id: frame.session_id,
            }),
            _status => Ok(MuxServerEvent::Unknown {
                session_id: frame.session_id,
            }),
        }
    }

    async fn write_new_stream_accepted<S>(
        &self,
        stream: &mut S,
        assigned_id: u16,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.write_new_stream_response(stream, assigned_id, MUX_STATUS_OK)
            .await
    }

    async fn write_new_stream_rejected<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.write_new_stream_response(stream, 0, MUX_STATUS_FAIL)
            .await
    }

    async fn write_new_stream_response<S>(
        &self,
        stream: &mut S,
        assigned_id: u16,
        status: u8,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let resp = encode_new_stream_response(assigned_id, status);
        stream
            .write_all(&resp)
            .await
            .map_err(|_| Error::Io("failed to write MUX new-stream response"))
    }

    /// Read next frame (with decryption for non-control frames).
    async fn recv<S>(&mut self, stream: &mut S) -> Result<MuxFrame, Error>
    where
        S: AsyncSocket,
    {
        let frame = read_mux_frame(stream).await?;
        self.decrypt_frame_c2s(frame)
    }

    /// Write data to a stream as a STATUS_KEEP frame.
    async fn write_data<S>(&mut self, stream: &mut S, sid: u16, data: &[u8]) -> Result<(), Error>
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

    /// Write an END frame for a stream.
    async fn write_end<S>(&mut self, stream: &mut S, sid: u16) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        let frame = encode_end_frame(sid);
        stream
            .write_all(&frame)
            .await
            .map_err(|_| Error::Io("failed to write MUX end frame"))
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
