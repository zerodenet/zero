//! Trojan outbound protocol handler.

use std::string::String;

#[cfg(feature = "tokio")]
use std::io;

#[cfg(feature = "tokio")]
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
#[cfg(feature = "tokio")]
use tokio::sync::{broadcast, mpsc};
use zero_core::{Address, Error, ProtocolType, Session, UdpFlowPacket};
use zero_traits::{
    AsyncSocket, TcpTunnelProtocol, UdpPacketStreamFraming, UdpPacketTunnelProtocol,
};

use super::shared::{CMD_TCP, CMD_UDP};

/// Trojan outbound handler.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanOutbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanTcpOutboundProfile {
    password: String,
}

impl TrojanTcpOutboundProfile {
    pub fn from_config_parts(password: impl Into<String>) -> Self {
        Self {
            password: password.into(),
        }
    }

    pub fn from_config_password(password: &str) -> Self {
        Self::from_config_parts(password)
    }

    pub async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        session: &Session,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        TrojanOutbound
            .establish_tcp_tunnel(stream, &TrojanTcpTunnelTarget::new(session, &self.password))
            .await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanTcpTlsProfile {
    server_name: Option<String>,
    insecure: bool,
    client_fingerprint: Option<String>,
}

impl TrojanTcpTlsProfile {
    pub fn from_config_parts(
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
    ) -> Self {
        Self {
            server_name: sni.map(ToOwned::to_owned),
            insecure,
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
        }
    }

    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    pub fn insecure(&self) -> bool {
        self.insecure
    }

    pub fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }
}

pub fn tcp_outbound_profile_from_config_password(password: &str) -> TrojanTcpOutboundProfile {
    TrojanTcpOutboundProfile::from_config_password(password)
}

pub fn tcp_tls_profile_from_config(
    sni: Option<&str>,
    insecure: bool,
    client_fingerprint: Option<&str>,
) -> TrojanTcpTlsProfile {
    TrojanTcpTlsProfile::from_config_parts(sni, insecure, client_fingerprint)
}

impl TrojanOutbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Trojan
    }

    /// Send the Trojan request over an established TLS stream.
    ///
    /// Writes: password hash + CRLF + CMD + address + port + CRLF.
    /// The upstream server then connects to the target and relays data.
    pub async fn send_request<S: AsyncSocket>(
        &self,
        stream: &mut S,
        session: &Session,
        password: &str,
    ) -> Result<(), Error> {
        let request = build_tcp_request(password, &session.target, session.port)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("trojan: write failed"))
    }
}

/// Target parameters for Trojan TCP tunnel.
#[derive(Debug, Clone, Copy)]
pub struct TrojanTcpTunnelTarget<'a> {
    pub session: &'a Session,
    pub password: &'a str,
}

impl<'a> TrojanTcpTunnelTarget<'a> {
    pub fn new(session: &'a Session, password: &'a str) -> Self {
        Self { session, password }
    }
}

impl<'a> TcpTunnelProtocol<TrojanTcpTunnelTarget<'a>> for TrojanOutbound {
    type Error = Error;

    async fn establish_tcp_tunnel<S>(
        &self,
        stream: &mut S,
        target: &TrojanTcpTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        self.send_request(stream, target.session, target.password)
            .await
    }
}

/// Target parameters for Trojan UDP packet tunnel over a connected stream.
#[derive(Debug, Clone, Copy)]
pub struct TrojanUdpPacketTunnelTarget<'a> {
    pub session: &'a Session,
    pub password: &'a str,
}

impl<'a> UdpPacketTunnelProtocol<TrojanUdpPacketTunnelTarget<'a>> for TrojanOutbound {
    type Error = Error;

    async fn establish_udp_packet_tunnel<S>(
        &self,
        stream: &mut S,
        target: &TrojanUdpPacketTunnelTarget<'a>,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        let request =
            build_udp_request(target.password, &target.session.target, target.session.port)?;
        stream
            .write_all(&request)
            .await
            .map_err(|_| Error::Io("trojan: write udp request failed"))
    }
}

/// One Trojan UDP packet carried over a connected stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanUdpPacket {
    target: Address,
    port: u16,
    payload: Vec<u8>,
}

impl TrojanUdpPacket {
    pub fn new(target: Address, port: u16, payload: Vec<u8>) -> Self {
        Self {
            target,
            port,
            payload,
        }
    }

    pub async fn write_to<S>(&self, stream: &mut S, flow_io: &TrojanUdpFlowIo) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        flow_io
            .write_packet(stream, &self.target, self.port, &self.payload)
            .await
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

#[derive(Debug, Default, Clone, Copy)]
pub struct TrojanUdpFlowIo;

#[cfg(feature = "tokio")]
type TrojanUdpFlowResponses = broadcast::Sender<UdpFlowPacket>;

#[cfg(feature = "tokio")]
pub type TrojanUdpFlowResponseReceiver = broadcast::Receiver<UdpFlowPacket>;

#[cfg(feature = "tokio")]
#[derive(Clone)]
struct TrojanUdpFlowSender {
    send_tx: mpsc::Sender<UdpFlowPacket>,
}

#[cfg(feature = "tokio")]
pub struct TrojanUdpFlowHandle {
    sender: TrojanUdpFlowSender,
    responses: TrojanUdpFlowResponses,
}

#[cfg(feature = "tokio")]
#[derive(Clone)]
pub struct TrojanUdpFlowSession {
    sender: TrojanUdpFlowSender,
    responses: TrojanUdpFlowResponses,
}

#[cfg(feature = "tokio")]
impl TrojanUdpFlowSession {
    pub fn new(handle: TrojanUdpFlowHandle) -> Self {
        Self {
            sender: handle.sender,
            responses: handle.responses,
        }
    }

    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        self.sender.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> TrojanUdpFlowResponseReceiver {
        self.responses.subscribe()
    }
}

#[cfg(feature = "tokio")]
#[derive(Clone)]
pub struct TrojanUdpFlowConnection {
    session: TrojanUdpFlowSession,
}

#[cfg(feature = "tokio")]
impl TrojanUdpFlowConnection {
    pub fn new(session: TrojanUdpFlowSession) -> Self {
        Self { session }
    }

    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        self.session.send(target, port, payload).await
    }

    pub fn subscribe_responses(&self) -> TrojanUdpFlowResponseReceiver {
        self.session.subscribe_responses()
    }
}

#[cfg(feature = "tokio")]
impl TrojanUdpFlowSender {
    pub async fn send(&self, target: &Address, port: u16, payload: &[u8]) -> Result<usize, Error> {
        let packet = UdpFlowPacket::from_parts(target, port, payload);
        let packet_len = packet.payload.len();
        self.send_tx
            .send(packet)
            .await
            .map_err(|_| Error::Io("trojan udp flow closed"))?;
        Ok(packet_len)
    }
}

impl TrojanUdpFlowIo {
    pub async fn establish<S>(
        &self,
        stream: &mut S,
        session: &Session,
        password: &str,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        establish_udp_packet_tunnel(stream, session, password).await
    }

    pub async fn establish_with_resume<S>(
        &self,
        stream: &mut S,
        session: &Session,
        resume: &TrojanUdpFlowResume,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        resume.establish_udp_tunnel(self, stream, session).await
    }

    pub async fn write_packet<S>(
        &self,
        stream: &mut S,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        write_udp_flow_packet(stream, target, port, payload).await
    }

    pub async fn read_packet<S>(&self, stream: &mut S) -> Result<TrojanUdpPacket, Error>
    where
        S: AsyncSocket,
    {
        read_udp_flow_packet(stream).await
    }

    pub async fn read_flow_packet<S>(&self, stream: &mut S) -> Result<UdpFlowPacket, Error>
    where
        S: AsyncSocket,
    {
        let packet = self.read_packet(stream).await?;
        let (target, port, payload) = packet.into_parts();
        Ok(UdpFlowPacket::new(target, port, payload))
    }

    pub async fn write_flow_packet<S>(
        &self,
        stream: &mut S,
        packet: &UdpFlowPacket,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.write_packet(stream, &packet.target, packet.port, &packet.payload)
            .await
    }

    pub async fn write_stream_packet<S>(
        &self,
        stream: &mut S,
        packet: &TrojanUdpPacket,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        packet.write_to(stream, self).await
    }

    pub async fn read_stream_packet<S>(&self, stream: &mut S) -> Result<TrojanUdpPacket, Error>
    where
        S: AsyncSocket,
    {
        self.read_packet(stream).await
    }
}

#[cfg(feature = "tokio")]
pub fn spawn_udp_flow<S>(stream: S) -> TrojanUdpFlowHandle
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
{
    let (read_half, write_half) = tokio::io::split(stream);
    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (recv_tx, _) = broadcast::channel::<UdpFlowPacket>(32);

    spawn_send_task(send_rx, WriteOnlySocket(write_half));
    spawn_recv_task(ReadOnlySocket(read_half), recv_tx.clone());

    TrojanUdpFlowHandle {
        sender: TrojanUdpFlowSender { send_tx },
        responses: recv_tx,
    }
}

#[cfg(feature = "tokio")]
pub async fn establish_udp_flow_with_resume<S>(
    mut stream: S,
    session: &Session,
    resume: &TrojanUdpFlowResume,
) -> Result<TrojanUdpFlowConnection, Error>
where
    S: AsyncSocket + tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
{
    let flow_io = TrojanUdpFlowIo;
    flow_io
        .establish_with_resume(&mut stream, session, resume)
        .await?;
    Ok(TrojanUdpFlowConnection::new(TrojanUdpFlowSession::new(
        spawn_udp_flow(stream),
    )))
}

#[cfg(feature = "tokio")]
fn spawn_send_task<S>(
    mut send_rx: mpsc::Receiver<UdpFlowPacket>,
    mut send_stream: WriteOnlySocket<S>,
) where
    S: tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
{
    tokio::spawn(async move {
        let flow_io = TrojanUdpFlowIo;
        while let Some(packet) = send_rx.recv().await {
            if flow_io
                .write_flow_packet(&mut send_stream, &packet)
                .await
                .is_err()
            {
                break;
            }
        }
    });
}

#[cfg(feature = "tokio")]
fn spawn_recv_task<S>(mut recv_stream: ReadOnlySocket<S>, recv_tx: broadcast::Sender<UdpFlowPacket>)
where
    S: tokio::io::AsyncRead + Send + Sync + Unpin + 'static,
{
    tokio::spawn(async move {
        let flow_io = TrojanUdpFlowIo;
        while let Ok(packet) = flow_io.read_flow_packet(&mut recv_stream).await {
            if recv_tx.send(packet).is_err() {
                break;
            }
        }
    });
}

#[cfg(feature = "tokio")]
struct ReadOnlySocket<S>(ReadHalf<S>);

#[cfg(feature = "tokio")]
impl<S> AsyncSocket for ReadOnlySocket<S>
where
    S: tokio::io::AsyncRead + Send + Sync + Unpin,
{
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.read(buf).await
    }

    async fn write_all(&mut self, _buf: &[u8]) -> Result<(), Self::Error> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "read-only socket cannot write",
        ))
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(feature = "tokio")]
struct WriteOnlySocket<S>(WriteHalf<S>);

#[cfg(feature = "tokio")]
impl<S> AsyncSocket for WriteOnlySocket<S>
where
    S: tokio::io::AsyncWrite + Send + Sync + Unpin,
{
    type Error = io::Error;

    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "write-only socket cannot read",
        ))
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.0.write_all(buf).await?;
        self.0.flush().await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.0.shutdown().await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanUdpFlowResume {
    password: String,
    sni: Option<String>,
    insecure: bool,
    client_fingerprint: Option<String>,
    relay_chain: bool,
}

impl TrojanUdpFlowResume {
    pub fn new(
        password: &str,
        sni: Option<&str>,
        insecure: bool,
        client_fingerprint: Option<&str>,
        relay_chain: bool,
    ) -> Self {
        Self {
            password: password.to_owned(),
            sni: sni.map(ToOwned::to_owned),
            insecure,
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
            relay_chain,
        }
    }

    pub fn flow_requires_relay_upstream(&self) -> bool {
        self.relay_chain
    }

    fn peer_config(&self) -> TrojanUdpPeerConfig<'_> {
        TrojanUdpPeerConfig {
            password: &self.password,
        }
    }

    fn leaf_cache_key(&self, server: &str, port: u16) -> TrojanUdpLeafKey {
        self.peer_config().leaf_cache_key(server, port)
    }

    fn flow_key(&self, server: &str, port: u16) -> TrojanUdpFlowKey {
        if self.relay_chain {
            TrojanUdpFlowKey::Relay
        } else {
            TrojanUdpFlowKey::Leaf(self.leaf_cache_key(server, port))
        }
    }

    fn cache_key(&self, server: &str, port: u16, session_id: u64) -> TrojanUdpCacheKey {
        TrojanUdpCacheKey::from_flow_key(self.flow_key(server, port), session_id)
    }

    pub fn flow_cache_key(&self, server: &str, port: u16, session_id: u64) -> String {
        if self.relay_chain {
            return format!("relay|session:{session_id}");
        }
        format!(
            "leaf|{server}:{port}|password:{}",
            self.peer_config().password
        )
    }

    pub fn connector_flow(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> TrojanUdpConnectorFlow {
        TrojanUdpConnectorFlow {
            cache_key: self.flow_cache_key(server, port, session_id),
            requires_relay_upstream: self.flow_requires_relay_upstream(),
        }
    }

    pub fn tls_profile(&self, fallback_server_name: Option<&str>) -> TrojanUdpTlsProfile {
        TrojanUdpTlsProfile {
            server_name: self
                .sni
                .as_deref()
                .or(fallback_server_name)
                .map(ToOwned::to_owned),
            insecure: self.insecure,
            client_fingerprint: self.client_fingerprint.clone(),
        }
    }

    pub fn tls_profile_spec(&self) -> TrojanUdpTlsProfileSpec {
        TrojanUdpTlsProfileSpec {
            sni: self.sni.clone(),
            insecure: self.insecure,
            client_fingerprint: self.client_fingerprint.clone(),
        }
    }

    pub async fn establish_udp_tunnel<S>(
        &self,
        flow_io: &TrojanUdpFlowIo,
        stream: &mut S,
        session: &Session,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        flow_io.establish(stream, session, &self.password).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanUdpConnectorFlow {
    cache_key: String,
    requires_relay_upstream: bool,
}

impl TrojanUdpConnectorFlow {
    pub fn into_parts(self) -> (String, bool) {
        (self.cache_key, self.requires_relay_upstream)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanUdpTlsProfileSpec {
    sni: Option<String>,
    insecure: bool,
    client_fingerprint: Option<String>,
}

impl TrojanUdpTlsProfileSpec {
    pub fn tls_profile(&self, fallback_server_name: Option<&str>) -> TrojanUdpTlsProfile {
        TrojanUdpTlsProfile {
            server_name: self
                .sni
                .as_deref()
                .or(fallback_server_name)
                .map(ToOwned::to_owned),
            insecure: self.insecure,
            client_fingerprint: self.client_fingerprint.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TrojanUdpFlowConfig<'a> {
    password: &'a str,
    sni: Option<&'a str>,
    insecure: bool,
    client_fingerprint: Option<&'a str>,
}

impl<'a> TrojanUdpFlowConfig<'a> {
    pub fn new(
        password: &'a str,
        sni: Option<&'a str>,
        insecure: bool,
        client_fingerprint: Option<&'a str>,
    ) -> Self {
        Self {
            password,
            sni,
            insecure,
            client_fingerprint,
        }
    }

    pub fn flow_resume(&self, relay_chain: bool) -> TrojanUdpFlowResume {
        TrojanUdpFlowResume::new(
            self.password,
            self.sni,
            self.insecure,
            self.client_fingerprint,
            relay_chain,
        )
    }
}

pub fn udp_flow_resume_from_config(
    password: &str,
    sni: Option<&str>,
    insecure: bool,
    client_fingerprint: Option<&str>,
    relay_chain: bool,
) -> TrojanUdpFlowResume {
    TrojanUdpFlowConfig::new(password, sni, insecure, client_fingerprint).flow_resume(relay_chain)
}

pub fn connector_flow_from_resume(
    resume: &TrojanUdpFlowResume,
    server: &str,
    port: u16,
    session_id: u64,
) -> TrojanUdpConnectorFlow {
    resume.connector_flow(server, port, session_id)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum TrojanUdpFlowKey {
    Leaf(TrojanUdpLeafKey),
    Relay,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum TrojanUdpCacheKey {
    Leaf(TrojanUdpLeafKey),
    Relay { session_id: u64 },
}

impl TrojanUdpCacheKey {
    fn from_flow_key(flow_key: TrojanUdpFlowKey, session_id: u64) -> Self {
        match flow_key {
            TrojanUdpFlowKey::Leaf(leaf_key) => Self::Leaf(leaf_key),
            TrojanUdpFlowKey::Relay => Self::Relay { session_id },
        }
    }
}

pub struct TrojanUdpFlowStore<T> {
    entries: std::collections::BTreeMap<TrojanUdpCacheKey, T>,
}

impl<T> Default for TrojanUdpFlowStore<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> TrojanUdpFlowStore<T> {
    pub fn new() -> Self {
        Self {
            entries: std::collections::BTreeMap::new(),
        }
    }

    pub fn get(
        &self,
        resume: &TrojanUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Option<&T> {
        let key = resume.cache_key(server, port, session_id);
        self.entries.get(&key)
    }

    pub fn insert(
        &mut self,
        resume: &TrojanUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
        value: T,
    ) -> Option<T> {
        let key = resume.cache_key(server, port, session_id);
        self.entries.insert(key, value)
    }
}

#[cfg(feature = "tokio")]
#[derive(Default)]
pub struct TrojanUdpFlowSessions {
    entries: TrojanUdpFlowStore<TrojanUdpFlowConnection>,
}

#[cfg(feature = "tokio")]
impl TrojanUdpFlowSessions {
    pub fn new() -> Self {
        Self {
            entries: TrojanUdpFlowStore::new(),
        }
    }

    pub fn get(
        &self,
        resume: &TrojanUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Option<&TrojanUdpFlowConnection> {
        self.entries.get(resume, server, port, session_id)
    }

    pub fn insert(
        &mut self,
        resume: &TrojanUdpFlowResume,
        server: &str,
        port: u16,
        session_id: u64,
        connection: TrojanUdpFlowConnection,
    ) -> Option<TrojanUdpFlowConnection> {
        self.entries
            .insert(resume, server, port, session_id, connection)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrojanUdpTlsProfile {
    server_name: Option<String>,
    insecure: bool,
    client_fingerprint: Option<String>,
}

impl TrojanUdpTlsProfile {
    pub fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    pub fn insecure(&self) -> bool {
        self.insecure
    }

    pub fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }
}

#[derive(Debug, Clone, Copy)]
struct TrojanUdpPeerConfig<'a> {
    password: &'a str,
}

impl<'a> TrojanUdpPeerConfig<'a> {
    fn leaf_cache_key(&self, server: &str, port: u16) -> TrojanUdpLeafKey {
        TrojanUdpLeafKey {
            server: server.to_owned(),
            port,
            password: self.password.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TrojanUdpLeafKey {
    server: String,
    port: u16,
    password: String,
}

impl UdpPacketStreamFraming<TrojanUdpPacket> for TrojanOutbound {
    type Error = Error;
    type Decoded = TrojanUdpPacket;

    async fn write_udp_packet<S>(
        &self,
        stream: &mut S,
        packet: &TrojanUdpPacket,
    ) -> Result<(), Self::Error>
    where
        S: AsyncSocket,
    {
        super::shared::write_udp_packet(stream, packet.target(), packet.port(), packet.payload())
            .await
            .map(|_| ())
    }

    async fn read_udp_packet<S>(&self, stream: &mut S) -> Result<Self::Decoded, Self::Error>
    where
        S: AsyncSocket,
    {
        let (target, port, payload) = super::shared::read_udp_packet(stream).await?;
        Ok(TrojanUdpPacket::new(target, port, payload))
    }
}

async fn read_inbound_udp_packet<S>(stream: &mut S) -> Result<TrojanUdpPacket, Error>
where
    S: AsyncSocket,
{
    <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::read_udp_packet(
        &TrojanOutbound,
        stream,
    )
    .await
}

async fn read_udp_flow_packet<S>(stream: &mut S) -> Result<TrojanUdpPacket, Error>
where
    S: AsyncSocket,
{
    read_inbound_udp_packet(stream).await
}

pub async fn establish_udp_packet_tunnel<S>(
    stream: &mut S,
    session: &Session,
    password: &str,
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    <TrojanOutbound as UdpPacketTunnelProtocol<TrojanUdpPacketTunnelTarget<'_>>>::establish_udp_packet_tunnel(
        &TrojanOutbound,
        stream,
        &TrojanUdpPacketTunnelTarget { session, password },
    )
    .await
}

async fn write_udp_response<S>(
    stream: &mut S,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    let packet = TrojanUdpPacket::new(target.clone(), port, payload.to_vec());
    <TrojanOutbound as UdpPacketStreamFraming<TrojanUdpPacket>>::write_udp_packet(
        &TrojanOutbound,
        stream,
        &packet,
    )
    .await
}

async fn write_udp_flow_packet<S>(
    stream: &mut S,
    target: &Address,
    port: u16,
    payload: &[u8],
) -> Result<(), Error>
where
    S: AsyncSocket,
{
    write_udp_response(stream, target, port, payload).await
}

/// Build a Trojan UDP associate request (CMD_UDP).
///
/// This is a standalone request builder used by the proxy outbound
/// module to initiate a UDP relay connection.
pub fn build_udp_request(password: &str, addr: &Address, port: u16) -> Result<Vec<u8>, Error> {
    build_trojan_request(password, addr, port, CMD_UDP)
}

fn build_tcp_request(password: &str, addr: &Address, port: u16) -> Result<Vec<u8>, Error> {
    build_trojan_request(password, addr, port, CMD_TCP)
}

fn build_trojan_request(
    password: &str,
    addr: &Address,
    port: u16,
    cmd: u8,
) -> Result<Vec<u8>, Error> {
    use super::shared::{ATYP_DOMAIN, ATYP_IPV4, ATYP_IPV6, CRLF};

    let mut request = Vec::new();

    #[cfg(feature = "crypto")]
    {
        use sha2::{Digest, Sha224};
        let digest = Sha224::digest(password.as_bytes());
        request.extend_from_slice(super::shared::hex::encode(&digest).as_bytes());
    }
    #[cfg(not(feature = "crypto"))]
    {
        let _ = password;
        return Err(Error::Unsupported("trojan: crypto feature not enabled"));
    }

    request.extend_from_slice(CRLF);
    request.push(cmd);

    match addr {
        Address::Ipv4(bytes) => {
            request.push(ATYP_IPV4);
            request.extend_from_slice(bytes);
        }
        Address::Ipv6(bytes) => {
            request.push(ATYP_IPV6);
            request.extend_from_slice(bytes);
        }
        Address::Domain(domain) => {
            let bytes = domain.as_bytes();
            if bytes.is_empty() || bytes.len() > 255 {
                return Err(Error::Protocol("trojan: domain too long"));
            }
            request.push(ATYP_DOMAIN);
            request.push(bytes.len() as u8);
            request.extend_from_slice(bytes);
        }
    }

    request.extend_from_slice(&port.to_be_bytes());
    request.extend_from_slice(CRLF);
    Ok(request)
}
