use core::future::Future;
use std::collections::HashMap;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::sync::mpsc;
use zero_core::{Address, Error, Network, ProtocolType, Session, SessionAuth};
use zero_traits::AsyncSocket;

use crate::outbound::VmessOutbound;
use crate::shared::VmessCipher;
use crate::shared::{parse_address_from_bytes, read_exact, write_address};
use crate::stream::VmessAeadStream;

pub const MUX_MAX_META_LEN: usize = 512;
pub const MUX_MAX_DATA_LEN: usize = 16 * 1024;
pub const MUX_NETWORK_TCP: u8 = 0x01;
pub const MUX_NETWORK_UDP: u8 = 0x02;
pub const MUX_STATUS_NEW: u8 = 0x01;
pub const MUX_STATUS_KEEP: u8 = 0x02;
pub const MUX_STATUS_END: u8 = 0x03;
pub const MUX_STATUS_KEEP_ALIVE: u8 = 0x04;
pub const MUX_OPTION_DATA: u8 = 0x01;
pub const MUX_OPTION_ERROR: u8 = 0x02;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VmessMuxPoolKey {
    pub server: String,
    pub port: u16,
    identity: VmessMuxIdentity,
    pub transport: VmessMuxTransportKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VmessMuxIdentity {
    uuid: [u8; 16],
    cipher_name: String,
    cipher: VmessCipher,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VmessMuxTransportKey {
    RawTls {
        server_name: Option<String>,
    },
    Ws {
        server_name: Option<String>,
        path: String,
    },
    Grpc {
        server_name: Option<String>,
        service_names: Vec<String>,
    },
}

#[derive(Clone)]
pub struct VmessMuxConnectionPool {
    pool: Arc<Mutex<HashMap<VmessMuxPoolKey, Arc<VmessMuxConn>>>>,
}

pub struct VmessMuxPoolKeyConfig {
    server: String,
    port: u16,
    identity: VmessMuxIdentity,
    tls_server_name: Option<String>,
    ws_path: Option<String>,
    grpc_service_names: Option<Vec<String>>,
}

impl VmessMuxPoolKeyConfig {
    pub fn new(server: impl Into<String>, port: u16, identity: VmessMuxIdentity) -> Self {
        Self {
            server: server.into(),
            port,
            identity,
            tls_server_name: None,
            ws_path: None,
            grpc_service_names: None,
        }
    }

    pub fn with_tls_server_name(mut self, server_name: Option<&str>) -> Self {
        self.tls_server_name = server_name.map(ToOwned::to_owned);
        self
    }

    pub fn with_ws_path(mut self, path: Option<&str>) -> Self {
        self.ws_path = path.map(ToOwned::to_owned);
        self
    }

    pub fn with_grpc_service_names(mut self, service_names: Option<Vec<String>>) -> Self {
        self.grpc_service_names = service_names;
        self
    }

    pub fn into_pool_key(self) -> Result<VmessMuxPoolKey, Error> {
        VmessMuxPoolKey::from_config_parts(
            self.server,
            self.port,
            self.identity,
            self.tls_server_name.as_deref(),
            self.ws_path.as_deref(),
            self.grpc_service_names,
        )
    }
}

impl VmessMuxIdentity {
    pub fn from_parts(id: [u8; 16], cipher_name: String, cipher: VmessCipher) -> Self {
        Self {
            uuid: id,
            cipher_name,
            cipher,
        }
    }

    pub fn uuid(&self) -> &[u8; 16] {
        &self.uuid
    }

    pub fn cipher(&self) -> VmessCipher {
        self.cipher
    }
}

impl VmessMuxPoolKey {
    pub fn from_identity(
        server: String,
        port: u16,
        identity: VmessMuxIdentity,
        transport: VmessMuxTransportKey,
    ) -> Self {
        Self {
            server,
            port,
            identity,
            transport,
        }
    }

    pub fn from_parts(
        server: String,
        port: u16,
        id: [u8; 16],
        cipher_name: String,
        cipher: VmessCipher,
        transport: VmessMuxTransportKey,
    ) -> Self {
        Self::from_identity(
            server,
            port,
            VmessMuxIdentity::from_parts(id, cipher_name, cipher),
            transport,
        )
    }

    pub fn from_config_parts(
        server: String,
        port: u16,
        identity: VmessMuxIdentity,
        tls_server_name: Option<&str>,
        ws_path: Option<&str>,
        grpc_service_names: Option<Vec<String>>,
    ) -> Result<Self, Error> {
        Ok(Self::from_identity(
            server,
            port,
            identity,
            transport_key_from_config(tls_server_name, ws_path, grpc_service_names)?,
        ))
    }

    pub fn uuid(&self) -> &[u8; 16] {
        self.identity.uuid()
    }

    pub fn cipher(&self) -> VmessCipher {
        self.identity.cipher()
    }

    pub fn endpoint(&self) -> (&str, u16) {
        (&self.server, self.port)
    }

    pub async fn establish_mux_outbound_stream<S>(
        &self,
        stream: S,
    ) -> Result<VmessAeadStream<S>, Error>
    where
        S: AsyncSocket,
    {
        establish_mux_outbound_stream(stream, self.uuid(), self.cipher()).await
    }

    pub fn into_pool_conn<S>(self, stream: S, max_concurrency: u32) -> VmessMuxConn
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        VmessMuxConn::new(stream, max_concurrency)
    }
}

impl Default for VmessMuxConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for VmessMuxConnectionPool {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VmessMuxConnectionPool")
            .field(
                "entries",
                &self.pool.lock().expect("vmess mux pool poisoned").len(),
            )
            .finish()
    }
}

impl VmessMuxConnectionPool {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn evict_all(&self) {
        self.pool.lock().expect("vmess mux pool poisoned").clear();
    }

    pub async fn get_or_create_conn<F, Fut, E>(
        &self,
        key: VmessMuxPoolKey,
        max_concurrency: u32,
        create_conn: F,
    ) -> Result<Arc<VmessMuxConn>, E>
    where
        F: FnOnce(VmessMuxPoolKey, u32) -> Fut,
        Fut: Future<Output = Result<VmessMuxConn, E>>,
    {
        let cached = self
            .pool
            .lock()
            .expect("vmess mux pool poisoned")
            .get(&key)
            .cloned();

        match cached {
            Some(conn) if conn.has_capacity() => Ok(conn),
            _ => {
                let conn = Arc::new(create_conn(key.clone(), max_concurrency).await?);
                self.pool
                    .lock()
                    .expect("vmess mux pool poisoned")
                    .insert(key, conn.clone());
                Ok(conn)
            }
        }
    }
}

pub fn transport_key_from_config(
    tls_server_name: Option<&str>,
    ws_path: Option<&str>,
    grpc_service_names: Option<Vec<String>>,
) -> Result<VmessMuxTransportKey, Error> {
    match (grpc_service_names, ws_path, tls_server_name) {
        (Some(service_names), None, server_name) => Ok(VmessMuxTransportKey::Grpc {
            server_name: server_name.map(ToOwned::to_owned),
            service_names,
        }),
        (None, Some(path), server_name) => Ok(VmessMuxTransportKey::Ws {
            server_name: server_name.map(ToOwned::to_owned),
            path: path.to_owned(),
        }),
        (None, None, server_name) => Ok(VmessMuxTransportKey::RawTls {
            server_name: server_name.map(ToOwned::to_owned),
        }),
        _ => Err(Error::Protocol("vmess: ws and grpc are mutually exclusive")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxFrame {
    pub session_id: u16,
    pub status: u8,
    pub option: u8,
    pub network: Option<Network>,
    pub target: Option<Address>,
    pub port: Option<u16>,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmessMuxServerEvent {
    KeepAlive,
    NewStream {
        session_id: u16,
        network: Network,
        target: Address,
        port: u16,
        payload: Vec<u8>,
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
        status: u8,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmessInboundMuxAction {
    KeepAlive,
    OpenStream {
        session_id: u16,
        session: Box<Session>,
        initial_payload: Vec<u8>,
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

pub struct VmessInboundMuxOpenedStream {
    session_id: u16,
    session: Box<Session>,
    up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

pub enum VmessInboundMuxOpenedKind {
    Tcp(VmessInboundMuxTcpOpenedStream),
    Udp(VmessInboundMuxUdpOpenedStream),
}

pub enum VmessInboundMuxOpenedRoute {
    Tcp {
        session_id: u16,
        session: Session,
        up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    },
    Udp {
        session_id: u16,
        port: u16,
        up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        responder: crate::udp::VmessInboundMuxUdpResponder,
    },
}

pub trait VmessInboundMuxOpenedRouteDispatcher {
    type Error;

    async fn dispatch_tcp_opened(
        &mut self,
        session_id: u16,
        session: Session,
        up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    ) -> Result<(), Self::Error>;

    async fn dispatch_udp_opened(
        &mut self,
        session_id: u16,
        up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        responder: crate::udp::VmessInboundMuxUdpResponder,
    ) -> Result<(), Self::Error>;
}

impl VmessInboundMuxOpenedRoute {
    pub async fn dispatch_with<D>(self, dispatcher: &mut D) -> Result<(), D::Error>
    where
        D: VmessInboundMuxOpenedRouteDispatcher,
    {
        match self {
            Self::Tcp {
                session_id,
                session,
                up_rx,
            } => {
                dispatcher
                    .dispatch_tcp_opened(session_id, session, up_rx)
                    .await
            }
            Self::Udp {
                session_id,
                up_rx,
                responder,
                ..
            } => {
                dispatcher
                    .dispatch_udp_opened(session_id, up_rx, responder)
                    .await
            }
        }
    }
}

pub enum VmessInboundMuxEvent {
    Opened(VmessInboundMuxOpenedStream),
}

impl VmessInboundMuxOpenedStream {
    pub fn new(
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

    pub fn into_parts(self) -> (u16, Session, mpsc::UnboundedReceiver<Vec<u8>>) {
        (self.session_id, *self.session, self.up_rx)
    }

    pub fn into_kind(self) -> VmessInboundMuxOpenedKind {
        let (session_id, session, up_rx) = self.into_parts();
        match session.network {
            Network::Tcp => VmessInboundMuxOpenedKind::Tcp(VmessInboundMuxTcpOpenedStream {
                session_id,
                session,
                up_rx,
            }),
            Network::Udp => VmessInboundMuxOpenedKind::Udp(VmessInboundMuxUdpOpenedStream {
                session_id,
                session,
                up_rx,
            }),
        }
    }

    pub fn into_route(self, writer: VmessInboundMuxWriter) -> VmessInboundMuxOpenedRoute {
        let (session_id, session, up_rx) = self.into_parts();
        match session.network {
            Network::Tcp => VmessInboundMuxOpenedRoute::Tcp {
                session_id,
                session,
                up_rx,
            },
            Network::Udp => {
                let port = session.port;
                let target = session.target;
                VmessInboundMuxOpenedRoute::Udp {
                    session_id,
                    port,
                    up_rx,
                    responder: crate::udp::VmessInboundMuxUdpResponder::new(
                        crate::udp::VmessInboundUdpSession::new(target, port),
                        writer,
                        session_id,
                    ),
                }
            }
        }
    }
}

pub struct VmessInboundMuxTcpOpenedStream {
    session_id: u16,
    session: Session,
    up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl VmessInboundMuxTcpOpenedStream {
    pub fn into_parts(self) -> (u16, Session, mpsc::UnboundedReceiver<Vec<u8>>) {
        (self.session_id, self.session, self.up_rx)
    }
}

pub struct VmessInboundMuxUdpOpenedStream {
    session_id: u16,
    session: Session,
    up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl VmessInboundMuxUdpOpenedStream {
    pub fn into_parts(self) -> (u16, Session, mpsc::UnboundedReceiver<Vec<u8>>) {
        (self.session_id, self.session, self.up_rx)
    }
}

pub fn mux_cool_session() -> Session {
    Session::new(
        0,
        Address::Domain(crate::shared::MUX_COOL_DOMAIN.to_owned()),
        crate::shared::MUX_COOL_PORT,
        Network::Tcp,
        zero_core::ProtocolType::Vmess,
    )
}

pub fn is_mux_cool_session(session: &Session) -> bool {
    matches!(&session.target, Address::Domain(domain) if domain == crate::shared::MUX_COOL_DOMAIN)
        && session.port == crate::shared::MUX_COOL_PORT
        && session.network == Network::Tcp
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmessInboundSessionKind {
    Tcp,
    Udp,
    Mux,
}

pub enum VmessInboundAcceptedStream<S> {
    Tcp {
        session: Session,
        stream: S,
    },
    Udp {
        session: Session,
        stream: S,
        responder: crate::udp::VmessInboundUdpResponder,
        auth: Option<SessionAuth>,
    },
    Mux {
        stream: S,
    },
}

pub trait VmessInboundAcceptedStreamDispatcher<S> {
    type Error;

    async fn dispatch_tcp_stream(&mut self, session: Session, stream: S)
        -> Result<(), Self::Error>;

    async fn dispatch_udp_stream(
        &mut self,
        session: Session,
        stream: S,
        responder: crate::udp::VmessInboundUdpResponder,
        auth: Option<SessionAuth>,
    ) -> Result<(), Self::Error>;

    async fn dispatch_mux_stream(
        &mut self,
        reader: tokio::io::ReadHalf<S>,
        mux_server: VmessInboundMuxServer,
    ) -> Result<(), Self::Error>;
}

pub fn classify_inbound_session(session: &Session) -> VmessInboundSessionKind {
    match session.network {
        Network::Udp => VmessInboundSessionKind::Udp,
        Network::Tcp if is_mux_cool_session(session) => VmessInboundSessionKind::Mux,
        Network::Tcp => VmessInboundSessionKind::Tcp,
    }
}

impl<S> VmessInboundAcceptedStream<S> {
    pub fn from_session_stream(session: Session, stream: S) -> Self {
        match classify_inbound_session(&session) {
            VmessInboundSessionKind::Tcp => Self::Tcp { session, stream },
            VmessInboundSessionKind::Udp => {
                let responder = crate::udp::VmessInboundUdpResponder::new(
                    crate::udp::VmessInboundUdpSession::new(session.target.clone(), session.port),
                );
                Self::Udp {
                    auth: session.auth.clone(),
                    session,
                    stream,
                    responder,
                }
            }
            VmessInboundSessionKind::Mux => Self::Mux { stream },
        }
    }

    pub async fn dispatch<Tcp, TcpFut, Udp, UdpFut, Mux, MuxFut, E>(
        self,
        tcp: Tcp,
        udp: Udp,
        mux: Mux,
    ) -> Result<(), E>
    where
        Tcp: FnOnce(Session, S) -> TcpFut,
        TcpFut: core::future::Future<Output = Result<(), E>>,
        Udp:
            FnOnce(Session, S, crate::udp::VmessInboundUdpResponder, Option<SessionAuth>) -> UdpFut,
        UdpFut: core::future::Future<Output = Result<(), E>>,
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        Mux: FnOnce(tokio::io::ReadHalf<S>, VmessInboundMuxServer) -> MuxFut,
        MuxFut: core::future::Future<Output = Result<(), E>>,
    {
        match self {
            Self::Tcp { session, stream } => tcp(session, stream).await,
            Self::Udp {
                session,
                stream,
                responder,
                auth,
            } => udp(session, stream, responder, auth).await,
            Self::Mux { stream } => {
                let (reader, writer) = tokio::io::split(stream);
                mux(
                    reader,
                    crate::inbound::VmessInbound.accept_mux_session_from_tokio_writer(writer),
                )
                .await
            }
        }
    }

    pub async fn dispatch_with<D>(self, dispatcher: &mut D) -> Result<(), D::Error>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
        D: VmessInboundAcceptedStreamDispatcher<S>,
    {
        match self {
            Self::Tcp { session, stream } => dispatcher.dispatch_tcp_stream(session, stream).await,
            Self::Udp {
                session,
                stream,
                responder,
                auth,
            } => {
                dispatcher
                    .dispatch_udp_stream(session, stream, responder, auth)
                    .await
            }
            Self::Mux { stream } => {
                let (reader, writer) = tokio::io::split(stream);
                dispatcher
                    .dispatch_mux_stream(
                        reader,
                        crate::inbound::VmessInbound.accept_mux_session_from_tokio_writer(writer),
                    )
                    .await
            }
        }
    }
}

pub fn encode_frame(
    session_id: u16,
    status: u8,
    option: u8,
    target: Option<(&Address, u16, Network)>,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let mut meta = Vec::new();
    meta.extend_from_slice(&session_id.to_be_bytes());
    meta.push(status);
    meta.push(option);

    if status == MUX_STATUS_NEW {
        let Some((address, port, network)) = target else {
            return Err(Error::Protocol("vmess mux new frame requires target"));
        };
        match network {
            Network::Tcp => meta.push(MUX_NETWORK_TCP),
            Network::Udp => meta.push(MUX_NETWORK_UDP),
        }
        meta.extend_from_slice(&port.to_be_bytes());
        write_address(&mut meta, address)?;
    }

    if meta.len() > MUX_MAX_META_LEN {
        return Err(Error::Protocol("vmess mux metadata too large"));
    }

    let mut frame = Vec::with_capacity(2 + meta.len() + 2 + payload.len());
    frame.extend_from_slice(&(meta.len() as u16).to_be_bytes());
    frame.extend_from_slice(&meta);
    if option & MUX_OPTION_DATA != 0 {
        if payload.len() > MUX_MAX_DATA_LEN {
            return Err(Error::Protocol("vmess mux payload too large"));
        }
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        frame.extend_from_slice(payload);
    }
    Ok(frame)
}

pub async fn read_frame<S: AsyncSocket>(stream: &mut S) -> Result<MuxFrame, Error> {
    let mut len_buf = [0u8; 2];
    read_exact(stream, &mut len_buf).await?;
    let meta_len = u16::from_be_bytes(len_buf) as usize;
    if meta_len > MUX_MAX_META_LEN {
        return Err(Error::Protocol("vmess mux metadata too large"));
    }

    let mut meta = vec![0_u8; meta_len];
    read_exact(stream, &mut meta).await?;
    let mut frame = decode_metadata(&meta)?;

    if frame.option & MUX_OPTION_DATA != 0 {
        read_exact(stream, &mut len_buf).await?;
        let data_len = u16::from_be_bytes(len_buf) as usize;
        if data_len > MUX_MAX_DATA_LEN {
            return Err(Error::Protocol("vmess mux data too large"));
        }
        frame.payload.resize(data_len, 0);
        if data_len > 0 {
            read_exact(stream, &mut frame.payload).await?;
        }
    }

    Ok(frame)
}

pub async fn read_frame_from_tokio<R>(reader: &mut R) -> Result<MuxFrame, Error>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut len_buf = [0u8; 2];
    tokio::io::AsyncReadExt::read_exact(reader, &mut len_buf)
        .await
        .map_err(|_| Error::Io("vmess: failed to read from socket"))?;
    let meta_len = u16::from_be_bytes(len_buf) as usize;
    if meta_len > MUX_MAX_META_LEN {
        return Err(Error::Protocol("vmess mux metadata too large"));
    }

    let mut meta = vec![0_u8; meta_len];
    tokio::io::AsyncReadExt::read_exact(reader, &mut meta)
        .await
        .map_err(|_| Error::Io("vmess: failed to read from socket"))?;
    let mut frame = decode_metadata(&meta)?;

    if frame.option & MUX_OPTION_DATA != 0 {
        tokio::io::AsyncReadExt::read_exact(reader, &mut len_buf)
            .await
            .map_err(|_| Error::Io("vmess: failed to read from socket"))?;
        let data_len = u16::from_be_bytes(len_buf) as usize;
        if data_len > MUX_MAX_DATA_LEN {
            return Err(Error::Protocol("vmess mux data too large"));
        }
        frame.payload.resize(data_len, 0);
        if data_len > 0 {
            tokio::io::AsyncReadExt::read_exact(reader, &mut frame.payload)
                .await
                .map_err(|_| Error::Io("vmess: failed to read from socket"))?;
        }
    }

    Ok(frame)
}

pub async fn read_mux_stream_frame<R>(reader: &mut R) -> Result<MuxFrame, Error>
where
    R: tokio::io::AsyncRead + Unpin,
{
    read_frame_from_tokio(reader).await
}

pub async fn read_mux_server_event<R>(reader: &mut R) -> Result<VmessMuxServerEvent, Error>
where
    R: tokio::io::AsyncRead + Unpin,
{
    read_mux_stream_frame(reader).await?.try_into_server_event()
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VmessInboundMuxSession;

#[derive(Debug, Default)]
pub struct VmessInboundMuxStreams {
    streams: std::collections::HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>,
}

impl VmessInboundMuxStreams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_stream(
        &mut self,
        session_id: u16,
        initial_payload: Vec<u8>,
    ) -> mpsc::UnboundedReceiver<Vec<u8>> {
        let (tx, rx) = mpsc::unbounded_channel::<Vec<u8>>();
        self.streams.insert(session_id, tx.clone());
        if !initial_payload.is_empty() {
            let _ = tx.send(initial_payload);
        }
        rx
    }

    pub fn push_stream_data(&self, session_id: u16, payload: Vec<u8>) -> bool {
        if payload.is_empty() {
            return true;
        }
        self.streams
            .get(&session_id)
            .is_some_and(|tx| tx.send(payload).is_ok())
    }

    pub fn close_inbound_stream(&mut self, session_id: u16) -> bool {
        self.streams
            .remove(&session_id)
            .is_some_and(|tx| tx.send(Vec::new()).is_ok())
    }

    pub fn apply_inbound_action(
        &mut self,
        action: VmessInboundMuxAction,
    ) -> Option<VmessInboundMuxOpenedStream> {
        match action {
            VmessInboundMuxAction::KeepAlive => None,
            VmessInboundMuxAction::OpenStream {
                session_id,
                session,
                initial_payload,
            } => {
                let up_rx = self.open_stream(session_id, initial_payload);
                Some(VmessInboundMuxOpenedStream::new(session_id, session, up_rx))
            }
            VmessInboundMuxAction::Data {
                session_id,
                payload,
            } => {
                let _ = self.push_stream_data(session_id, payload);
                None
            }
            VmessInboundMuxAction::End { session_id } => {
                let _ = self.close_inbound_stream(session_id);
                None
            }
            VmessInboundMuxAction::Unknown { .. } => None,
        }
    }
}

pub async fn relay_inbound_mux_stream<S>(
    session_id: u16,
    mut up_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    writer: VmessInboundMuxWriter,
    mut upstream: S,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mux_session = VmessInboundMuxSession::new();
    let mut buf = vec![0_u8; MUX_MAX_DATA_LEN];
    loop {
        tokio::select! {
            payload = up_rx.recv() => {
                let Some(payload) = payload else { break; };
                if payload.is_empty() {
                    break;
                }
                if tokio::io::AsyncWriteExt::write_all(&mut upstream, &payload).await.is_err() {
                    break;
                }
                if tokio::io::AsyncWriteExt::flush(&mut upstream).await.is_err() {
                    break;
                }
            }
            read = tokio::io::AsyncReadExt::read(&mut upstream, &mut buf) => {
                match read {
                    Ok(0) => break,
                    Ok(n) => {
                        if mux_session
                            .write_inbound_stream_payload(&writer, session_id, &buf[..n])
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
    let _ = mux_session.write_inbound_stream_payload(&writer, session_id, &[]);
}

pub struct VmessInboundMuxServer {
    session: VmessInboundMuxSession,
    streams: VmessInboundMuxStreams,
    writer: VmessInboundMuxWriter,
}

impl VmessInboundMuxServer {
    pub fn from_tokio_writer<W>(writer: W) -> Self
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        Self {
            session: VmessInboundMuxSession::new(),
            streams: VmessInboundMuxStreams::new(),
            writer: VmessInboundMuxWriter::from_tokio_writer(writer),
        }
    }

    pub async fn read_opened_stream<R>(
        &mut self,
        reader: &mut R,
    ) -> Result<Option<VmessInboundMuxOpenedStream>, Error>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let action = self.session.read_inbound_action(reader).await?;
        Ok(self.streams.apply_inbound_action(action))
    }

    pub async fn next_opened_stream<R>(
        &mut self,
        reader: &mut R,
    ) -> Result<Option<VmessInboundMuxEvent>, Error>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        self.read_opened_stream(reader)
            .await
            .map(|opened| opened.map(VmessInboundMuxEvent::Opened))
    }

    pub async fn next_opened_route<R>(
        &mut self,
        reader: &mut R,
    ) -> Result<Option<VmessInboundMuxOpenedRoute>, Error>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        let writer = self.writer();
        self.next_opened_stream(reader).await.map(|event| {
            event.map(|event| match event {
                VmessInboundMuxEvent::Opened(opened) => opened.into_route(writer),
            })
        })
    }

    pub async fn dispatch_next_opened_route<R, D>(
        &mut self,
        reader: &mut R,
        dispatcher: &mut D,
    ) -> Result<bool, D::Error>
    where
        R: tokio::io::AsyncRead + Unpin,
        D: VmessInboundMuxOpenedRouteDispatcher,
        D::Error: From<Error>,
    {
        let Some(route) = self.next_opened_route(reader).await? else {
            return Ok(true);
        };
        route.dispatch_with(dispatcher).await?;
        Ok(true)
    }

    pub fn writer(&self) -> VmessInboundMuxWriter {
        self.writer.clone()
    }

    pub fn end_inbound_stream(&self, session_id: u16) -> Result<usize, Error> {
        self.session.end_inbound_stream(&self.writer, session_id)
    }
}

impl crate::inbound::VmessInbound {
    pub fn accept_mux_session_from_tokio_writer<W>(&self, writer: W) -> VmessInboundMuxServer
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        VmessInboundMuxServer::from_tokio_writer(writer)
    }
}

impl VmessInboundMuxSession {
    pub fn new() -> Self {
        Self
    }

    pub async fn next_action<R>(&self, reader: &mut R) -> Result<VmessInboundMuxAction, Error>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        read_mux_server_event(reader).await.map(Into::into)
    }

    pub async fn read_inbound_action<R>(
        &self,
        reader: &mut R,
    ) -> Result<VmessInboundMuxAction, Error>
    where
        R: tokio::io::AsyncRead + Unpin,
    {
        self.next_action(reader).await
    }

    pub fn write_data(
        &self,
        writer: &VmessInboundMuxWriter,
        session_id: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        writer.data(session_id, payload)
    }

    pub fn write_inbound_stream_data(
        &self,
        writer: &VmessInboundMuxWriter,
        session_id: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        self.write_data(writer, session_id, payload)
    }

    pub fn write_inbound_stream_payload(
        &self,
        writer: &VmessInboundMuxWriter,
        session_id: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        if payload.is_empty() {
            self.end_inbound_stream(writer, session_id)
        } else {
            self.write_inbound_stream_data(writer, session_id, payload)
        }
    }

    pub fn write_end(
        &self,
        writer: &VmessInboundMuxWriter,
        session_id: u16,
    ) -> Result<usize, Error> {
        writer.end(session_id)
    }

    pub fn end_inbound_stream(
        &self,
        writer: &VmessInboundMuxWriter,
        session_id: u16,
    ) -> Result<usize, Error> {
        self.write_end(writer, session_id)
    }
}

#[derive(Clone)]
pub struct VmessInboundMuxWriter {
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
}

impl VmessInboundMuxWriter {
    pub fn new(write_tx: mpsc::UnboundedSender<Vec<u8>>) -> Self {
        Self { write_tx }
    }

    pub fn from_tokio_writer<W>(writer: W) -> Self
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        let (write_tx, write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        spawn_mux_write_relay(writer, write_rx);
        Self::new(write_tx)
    }

    pub fn data(&self, session_id: u16, payload: &[u8]) -> Result<usize, Error> {
        queue_keep_stream(&self.write_tx, session_id, payload)
    }

    pub fn end(&self, session_id: u16) -> Result<usize, Error> {
        queue_end_stream(&self.write_tx, session_id)
    }

    pub fn end_inbound_stream(&self, session_id: u16) -> Result<usize, Error> {
        self.end(session_id)
    }

    pub fn write_inbound_stream_payload(
        &self,
        session_id: u16,
        payload: &[u8],
    ) -> Result<usize, Error> {
        if payload.is_empty() {
            self.end_inbound_stream(session_id)
        } else {
            self.data(session_id, payload)
        }
    }

    pub(crate) fn frame(&self, frame: Vec<u8>) -> Result<usize, Error> {
        let len = frame.len();
        self.write_tx
            .send(frame)
            .map_err(|_| Error::Io("failed to queue VMess MUX frame"))?;
        Ok(len)
    }
}

pub fn decode_metadata(meta: &[u8]) -> Result<MuxFrame, Error> {
    if meta.len() < 4 {
        return Err(Error::Protocol("vmess mux metadata too short"));
    }

    let session_id = u16::from_be_bytes([meta[0], meta[1]]);
    let status = meta[2];
    let option = meta[3];

    let mut frame = MuxFrame {
        session_id,
        status,
        option,
        network: None,
        target: None,
        port: None,
        payload: Vec::new(),
    };

    if status == MUX_STATUS_NEW {
        if meta.len() < 8 {
            return Err(Error::Protocol("vmess mux new metadata too short"));
        }
        frame.network = match meta[4] {
            MUX_NETWORK_TCP => Some(Network::Tcp),
            MUX_NETWORK_UDP => Some(Network::Udp),
            _ => return Err(Error::Protocol("vmess mux unknown network")),
        };
        frame.port = Some(u16::from_be_bytes([meta[5], meta[6]]));
        frame.target = Some(parse_address_from_bytes(meta[7], &meta[8..])?);
    }

    Ok(frame)
}

pub fn encode_open_stream(
    session_id: u16,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    encode_open_stream_with_network(session_id, target, port, Network::Tcp, payload)
}

pub fn encode_open_stream_with_network(
    session_id: u16,
    target: &Address,
    port: u16,
    network: Network,
    payload: &[u8],
) -> Result<Vec<u8>, Error> {
    let option = if payload.is_empty() {
        0
    } else {
        MUX_OPTION_DATA
    };
    encode_frame(
        session_id,
        MUX_STATUS_NEW,
        option,
        Some((target, port, network)),
        payload,
    )
}

pub fn encode_keep_stream(session_id: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
    encode_frame(session_id, MUX_STATUS_KEEP, MUX_OPTION_DATA, None, payload)
}

pub fn encode_end_stream(session_id: u16) -> Result<Vec<u8>, Error> {
    encode_frame(session_id, MUX_STATUS_END, 0, None, &[])
}

impl MuxFrame {
    pub fn try_into_server_event(self) -> Result<VmessMuxServerEvent, Error> {
        match self.status {
            MUX_STATUS_KEEP_ALIVE => Ok(VmessMuxServerEvent::KeepAlive),
            MUX_STATUS_NEW => {
                let network = self
                    .network
                    .ok_or(Error::Protocol("vmess mux new frame missing network"))?;
                let target = self
                    .target
                    .ok_or(Error::Protocol("vmess mux new frame missing target"))?;
                let port = self
                    .port
                    .ok_or(Error::Protocol("vmess mux new frame missing port"))?;
                Ok(VmessMuxServerEvent::NewStream {
                    session_id: self.session_id,
                    network,
                    target,
                    port,
                    payload: self.payload,
                })
            }
            MUX_STATUS_KEEP => Ok(VmessMuxServerEvent::Data {
                session_id: self.session_id,
                payload: self.payload,
            }),
            MUX_STATUS_END => Ok(VmessMuxServerEvent::End {
                session_id: self.session_id,
            }),
            status => Ok(VmessMuxServerEvent::Unknown {
                session_id: self.session_id,
                status,
            }),
        }
    }
}

impl From<VmessMuxServerEvent> for VmessInboundMuxAction {
    fn from(event: VmessMuxServerEvent) -> Self {
        match event {
            VmessMuxServerEvent::KeepAlive => Self::KeepAlive,
            VmessMuxServerEvent::NewStream {
                session_id,
                network,
                target,
                port,
                payload,
            } => Self::OpenStream {
                session_id,
                session: Box::new(Session::new(0, target, port, network, ProtocolType::Vmess)),
                initial_payload: payload,
            },
            VmessMuxServerEvent::Data {
                session_id,
                payload,
            } => Self::Data {
                session_id,
                payload,
            },
            VmessMuxServerEvent::End { session_id } => Self::End { session_id },
            VmessMuxServerEvent::Unknown { session_id, .. } => Self::Unknown { session_id },
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct VmessMuxFrameEncoder;

impl VmessMuxFrameEncoder {
    pub fn keep_stream(&self, session_id: u16, payload: &[u8]) -> Result<Vec<u8>, Error> {
        encode_keep_stream(session_id, payload)
    }

    pub fn end_stream(&self, session_id: u16) -> Result<Vec<u8>, Error> {
        encode_end_stream(session_id)
    }
}

pub fn queue_keep_stream(
    write_tx: &mpsc::UnboundedSender<Vec<u8>>,
    session_id: u16,
    payload: &[u8],
) -> Result<usize, Error> {
    let frame = encode_keep_stream(session_id, payload)?;
    let len = frame.len();
    write_tx
        .send(frame)
        .map_err(|_| Error::Io("failed to queue VMess MUX keep frame"))?;
    Ok(len)
}

pub fn queue_end_stream(
    write_tx: &mpsc::UnboundedSender<Vec<u8>>,
    session_id: u16,
) -> Result<usize, Error> {
    let frame = encode_end_stream(session_id)?;
    let len = frame.len();
    write_tx
        .send(frame)
        .map_err(|_| Error::Io("failed to queue VMess MUX end frame"))?;
    Ok(len)
}

pub struct VmessMuxStream {
    session_id: u16,
    target: Address,
    port: u16,
    network: Network,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    write_buf: Vec<u8>,
    write_pos: usize,
    read_buf: Vec<u8>,
    read_pos: usize,
    opened: bool,
    ended: bool,
    active: Option<Arc<Mutex<usize>>>,
}

pub struct VmessMuxConn {
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    streams: Arc<Mutex<std::collections::HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>>,
    next_id: Mutex<u16>,
    active: Arc<Mutex<usize>>,
    max_concurrency: u32,
}

impl VmessMuxConn {
    pub fn new<S>(stream: S, max_concurrency: u32) -> Self
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (reader, writer) = tokio::io::split(stream);
        let (write_tx, write_rx) = mpsc::unbounded_channel::<Vec<u8>>();
        let streams = Arc::new(Mutex::new(std::collections::HashMap::new()));

        spawn_mux_write_relay(writer, write_rx);
        spawn_mux_read_relay(reader, streams.clone());

        Self {
            write_tx,
            streams,
            next_id: Mutex::new(1),
            active: Arc::new(Mutex::new(0)),
            max_concurrency,
        }
    }

    pub fn has_capacity(&self) -> bool {
        *self.active.lock().unwrap() < self.max_concurrency as usize
    }

    pub fn open_stream(&self, target: Address, port: u16, network: Network) -> VmessMuxStream {
        let session_id = self.allocate_stream_id();
        let (down_tx, down_rx) = mpsc::unbounded_channel();
        self.streams.lock().unwrap().insert(session_id, down_tx);

        VmessMuxStream::new_with_network(
            session_id,
            target,
            port,
            network,
            self.write_tx.clone(),
            down_rx,
            self.active.clone(),
        )
    }

    fn allocate_stream_id(&self) -> u16 {
        let session_id = {
            let mut next = self.next_id.lock().unwrap();
            let id = *next;
            *next = next.wrapping_add(1);
            if *next == 0 {
                *next = 1;
            }
            id
        };
        *self.active.lock().unwrap() += 1;
        session_id
    }
}

fn spawn_mux_write_relay<W>(mut writer: W, mut write_rx: mpsc::UnboundedReceiver<Vec<u8>>)
where
    W: AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        while let Some(frame) = write_rx.recv().await {
            if writer.write_all(&frame).await.is_err() {
                break;
            }
            if writer.flush().await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    });
}

fn spawn_mux_read_relay<R>(
    mut reader: R,
    streams: Arc<Mutex<std::collections::HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>>,
) where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            let event = match read_mux_server_event(&mut reader).await {
                Ok(event) => event,
                Err(_) => break,
            };
            match event {
                VmessMuxServerEvent::KeepAlive => continue,
                VmessMuxServerEvent::Data {
                    session_id,
                    payload,
                }
                | VmessMuxServerEvent::NewStream {
                    session_id,
                    payload,
                    ..
                } => {
                    let tx = streams.lock().unwrap().get(&session_id).cloned();
                    if let Some(tx) = tx {
                        if !payload.is_empty() {
                            let _ = tx.send(payload);
                        }
                    }
                }
                VmessMuxServerEvent::End { session_id } => {
                    let tx = streams.lock().unwrap().get(&session_id).cloned();
                    if let Some(tx) = tx {
                        let _ = tx.send(Vec::new());
                        streams.lock().unwrap().remove(&session_id);
                    }
                }
                VmessMuxServerEvent::Unknown { session_id, .. } => {
                    let tx = streams.lock().unwrap().get(&session_id).cloned();
                    if let Some(tx) = tx {
                        let _ = tx.send(Vec::new());
                        streams.lock().unwrap().remove(&session_id);
                    }
                }
            }
        }
    });
}

impl VmessMuxStream {
    pub fn new(
        session_id: u16,
        target: Address,
        port: u16,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        active: Arc<Mutex<usize>>,
    ) -> Self {
        Self::new_with_network(
            session_id,
            target,
            port,
            Network::Tcp,
            write_tx,
            read_rx,
            active,
        )
    }

    pub fn new_with_network(
        session_id: u16,
        target: Address,
        port: u16,
        network: Network,
        write_tx: mpsc::UnboundedSender<Vec<u8>>,
        read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
        active: Arc<Mutex<usize>>,
    ) -> Self {
        Self {
            session_id,
            target,
            port,
            network,
            write_tx,
            read_rx,
            write_buf: Vec::new(),
            write_pos: 0,
            read_buf: Vec::new(),
            read_pos: 0,
            opened: false,
            ended: false,
            active: Some(active),
        }
    }

    fn queue_frame(&mut self, payload: &[u8]) -> io::Result<usize> {
        let take = payload.len().min(MUX_MAX_DATA_LEN);
        let frame = if self.opened {
            encode_keep_stream(self.session_id, &payload[..take])
        } else {
            self.opened = true;
            encode_open_stream_with_network(
                self.session_id,
                &self.target,
                self.port,
                self.network,
                &payload[..take],
            )
        }
        .map_err(protocol_error)?;
        self.write_tx
            .send(frame)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "vmess mux writer closed"))?;
        Ok(take)
    }

    fn flush_pending(&mut self) -> io::Result<()> {
        if self.write_pos < self.write_buf.len() {
            self.write_tx
                .send(self.write_buf[self.write_pos..].to_vec())
                .map_err(|_| {
                    io::Error::new(io::ErrorKind::BrokenPipe, "vmess mux writer closed")
                })?;
            self.write_pos = self.write_buf.len();
        }
        self.write_buf.clear();
        self.write_pos = 0;
        Ok(())
    }
}

pub fn mux_stream_with_network(
    session_id: u16,
    target: Address,
    port: u16,
    network: Network,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    read_rx: mpsc::UnboundedReceiver<Vec<u8>>,
    active: Arc<Mutex<usize>>,
) -> VmessMuxStream {
    VmessMuxStream::new_with_network(session_id, target, port, network, write_tx, read_rx, active)
}

pub async fn establish_mux_outbound_stream<S>(
    mut stream: S,
    uuid: &[u8; 16],
    cipher: crate::shared::VmessCipher,
) -> Result<VmessAeadStream<S>, Error>
where
    S: AsyncSocket,
{
    let mux_session = VmessOutbound
        .establish_tcp_session(&mut stream, &mux_cool_session(), uuid, cipher)
        .await?;
    VmessAeadStream::outbound(stream, mux_session)
}

impl Drop for VmessMuxStream {
    fn drop(&mut self) {
        if !self.ended {
            if !self.opened {
                let _ = self.write_tx.send(
                    encode_open_stream_with_network(
                        self.session_id,
                        &self.target,
                        self.port,
                        self.network,
                        &[],
                    )
                    .unwrap_or_default(),
                );
            }
            let _ = self
                .write_tx
                .send(encode_end_stream(self.session_id).unwrap_or_default());
            self.ended = true;
        }
        if let Some(active) = self.active.take() {
            if let Ok(mut count) = active.lock() {
                *count = count.saturating_sub(1);
            }
        }
    }
}

impl AsyncRead for VmessMuxStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.read_pos < self.read_buf.len() {
            let n = (self.read_buf.len() - self.read_pos).min(buf.remaining());
            buf.put_slice(&self.read_buf[self.read_pos..self.read_pos + n]);
            self.read_pos += n;
            if self.read_pos == self.read_buf.len() {
                self.read_buf.clear();
                self.read_pos = 0;
            }
            return Poll::Ready(Ok(()));
        }

        match Pin::new(&mut self.read_rx).poll_recv(cx) {
            Poll::Ready(Some(chunk)) => {
                if chunk.is_empty() {
                    self.ended = true;
                    return Poll::Ready(Ok(()));
                }
                let n = chunk.len().min(buf.remaining());
                buf.put_slice(&chunk[..n]);
                if n < chunk.len() {
                    self.read_buf = chunk;
                    self.read_pos = n;
                }
                Poll::Ready(Ok(()))
            }
            Poll::Ready(None) => {
                self.ended = true;
                Poll::Ready(Ok(()))
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl AsyncWrite for VmessMuxStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if let Err(error) = self.flush_pending() {
            return Poll::Ready(Err(error));
        }
        if buf.is_empty() {
            return Poll::Ready(Ok(0));
        }
        Poll::Ready(self.queue_frame(buf))
    }

    fn poll_flush(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(self.flush_pending())
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if let Err(error) = self.flush_pending() {
            return Poll::Ready(Err(error));
        }
        if !self.ended {
            if !self.opened {
                match encode_open_stream_with_network(
                    self.session_id,
                    &self.target,
                    self.port,
                    self.network,
                    &[],
                ) {
                    Ok(frame) => {
                        if self.write_tx.send(frame).is_err() {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::BrokenPipe,
                                "vmess mux writer closed",
                            )));
                        }
                        self.opened = true;
                    }
                    Err(error) => return Poll::Ready(Err(protocol_error(error))),
                }
            }
            match encode_end_stream(self.session_id) {
                Ok(frame) => {
                    if self.write_tx.send(frame).is_err() {
                        return Poll::Ready(Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "vmess mux writer closed",
                        )));
                    }
                }
                Err(error) => return Poll::Ready(Err(protocol_error(error))),
            }
            self.ended = true;
        }
        Poll::Ready(Ok(()))
    }
}

fn protocol_error(error: Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
