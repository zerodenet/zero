#[cfg(feature = "hysteria2")]
use core::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
#[cfg(feature = "hysteria2")]
use tokio::task::JoinSet;
use zero_config::InboundProtocolConfig;
#[cfg(feature = "hysteria2")]
use zero_core::{Address, InboundClientResponse, Session};
use zero_engine::EngineError;
#[cfg(feature = "hysteria2")]
use zero_engine::ResolvedLeafOutbound;
use zero_traits::AsyncSocket;
#[cfg(feature = "hysteria2")]
use zero_traits::DatagramCodec;

#[cfg(feature = "hysteria2")]
use crate::managed_udp::ManagedTupleUdpConnectionOps;

/// Bidirectional QUIC stream wrapper used by Hysteria2 proxy glue.
pub struct Hysteria2Stream {
    send: quinn::SendStream,
    recv: quinn::RecvStream,
}

impl Hysteria2Stream {
    pub fn new(send: quinn::SendStream, recv: quinn::RecvStream) -> Self {
        Self { send, recv }
    }
}

impl AsyncRead for Hysteria2Stream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl AsyncWrite for Hysteria2Stream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        Pin::new(&mut self.send)
            .poll_write(cx, buf)
            .map_err(io::Error::other)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.send)
            .poll_flush(cx)
            .map_err(io::Error::other)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), io::Error>> {
        Pin::new(&mut self.send)
            .poll_shutdown(cx)
            .map_err(io::Error::other)
    }
}

impl AsyncSocket for Hysteria2Stream {
    type Error = io::Error;

    fn read<'a>(
        &'a mut self,
        buf: &'a mut [u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>> + Send + 'a {
        async move { AsyncReadExt::read(self, buf).await }
    }

    fn write_all<'a>(
        &'a mut self,
        buf: &'a [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move {
            AsyncWriteExt::write_all(self, buf).await?;
            AsyncWriteExt::flush(self).await
        }
    }

    fn shutdown<'a>(
        &'a mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> + Send + 'a {
        async move { AsyncWriteExt::shutdown(self).await }
    }
}

#[derive(Debug, Clone)]
pub struct OwnedHysteria2InboundBindPlan {
    cert_path: String,
    key_path: String,
    source_dir: Option<PathBuf>,
}

impl OwnedHysteria2InboundBindPlan {
    pub fn from_config_ref(
        source_dir: Option<&Path>,
        cert_path: Option<&str>,
        key_path: Option<&str>,
    ) -> Self {
        Self {
            cert_path: cert_path.unwrap_or("certs/fullchain.pem").to_owned(),
            key_path: key_path.unwrap_or("certs/privkey.pem").to_owned(),
            source_dir: source_dir.map(PathBuf::from),
        }
    }

    pub fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        match protocol {
            InboundProtocolConfig::Hysteria2 {
                cert_path,
                key_path,
                ..
            } => Ok(Self::from_config_ref(
                source_dir,
                cert_path.as_deref(),
                key_path.as_deref(),
            )),
            _ => Err(EngineError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "hysteria2 inbound bind plan received non-hysteria2 inbound config",
            ))),
        }
    }

    pub async fn bind(&self, listen_addr: &str) -> Result<crate::quic::QuicInbound, EngineError> {
        crate::quic::QuicInbound::bind(
            listen_addr,
            &self.cert_path,
            &self.key_path,
            self.source_dir.as_deref(),
        )
        .await
    }
}

#[async_trait::async_trait]
impl crate::inbound_route::ProtocolInboundBindPlan for OwnedHysteria2InboundBindPlan {
    fn from_protocol_config(
        protocol: &InboundProtocolConfig,
        source_dir: Option<&Path>,
    ) -> Result<Self, EngineError> {
        Self::from_protocol_config(protocol, source_dir)
    }

    async fn bind(
        &self,
        listen_addr: &str,
    ) -> Result<crate::inbound_route::TransportInboundBindTarget, EngineError> {
        Ok(crate::inbound_route::TransportInboundBindTarget::Quic(
            OwnedHysteria2InboundBindPlan::bind(self, listen_addr).await?,
        ))
    }
}

pub struct QuicConnectionOptions<'a> {
    pub server: &'a str,
    pub port: u16,
    pub alpn: Vec<Vec<u8>>,
    pub quic_profile: Hysteria2QuicProfile,
    pub datagram_receive_buffer_size: Option<usize>,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone)]
pub struct Hysteria2ManagedDatagramFlowResume {
    protocol: hysteria2::udp::Hysteria2UdpFlowResume,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone)]
pub struct OwnedHysteria2InboundProfile {
    protocol: hysteria2::inbound::Hysteria2InboundProfile,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, Copy, Default)]
pub struct OwnedHysteria2InboundTcpResponseProtocol {
    protocol: hysteria2::inbound::Hysteria2InboundTcpAcceptor,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2ManagedUdpPacketPathCarrierDescriptor {
    protocol: hysteria2::udp::Hysteria2UdpPacketPathCarrierDescriptor,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2ManagedUdpPacketPathCarrierBuild {
    protocol: hysteria2::udp::Hysteria2UdpPacketPathCarrierBuild,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone)]
pub struct Hysteria2ManagedUdpFlowPlan<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    resume: Hysteria2ManagedDatagramFlowResume,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone)]
pub struct Hysteria2ManagedUdpPacketPathPlan {
    carrier_descriptor: Hysteria2ManagedUdpPacketPathCarrierDescriptor,
    carrier_build: Hysteria2ManagedUdpPacketPathCarrierBuild,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, Copy)]
pub struct Hysteria2ManagedUdpFlowConfig<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    password: &'a str,
    client_fingerprint: Option<&'a str>,
}

#[cfg(feature = "hysteria2")]
#[derive(Debug, Clone, Copy)]
pub struct Hysteria2TransportLeaf<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    password: &'a str,
    client_fingerprint: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hysteria2QuicProfile {
    client_fingerprint: Option<String>,
}

#[cfg(feature = "hysteria2")]
pub fn inbound_profile_from_protocol(
    protocol: &InboundProtocolConfig,
) -> Result<OwnedHysteria2InboundProfile, EngineError> {
    match protocol {
        InboundProtocolConfig::Hysteria2 { password, .. } => Ok(OwnedHysteria2InboundProfile::new(
            hysteria2::inbound::inbound_profile_from_config_password(password.as_str()),
        )),
        _ => Err(EngineError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "hysteria2 inbound profile received non-hysteria2 inbound config",
        ))),
    }
}

#[cfg(feature = "hysteria2")]
pub fn inbound_tcp_acceptor() -> OwnedHysteria2InboundTcpResponseProtocol {
    OwnedHysteria2InboundTcpResponseProtocol {
        protocol: hysteria2::inbound::Hysteria2InboundTcpAcceptor::new(),
    }
}

#[cfg(feature = "hysteria2")]
impl OwnedHysteria2InboundProfile {
    fn new(protocol: hysteria2::inbound::Hysteria2InboundProfile) -> Self {
        Self { protocol }
    }

    pub fn tcp_response_protocol(&self) -> OwnedHysteria2InboundTcpResponseProtocol {
        inbound_tcp_acceptor()
    }
}

#[cfg(feature = "hysteria2")]
impl<S> InboundClientResponse<S> for OwnedHysteria2InboundTcpResponseProtocol
where
    S: AsyncSocket,
{
    async fn send_ok(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_ok(client).await
    }

    async fn send_blocked(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_blocked(client).await
    }

    async fn send_upstream_failure(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_upstream_failure(client).await
    }
}

impl Hysteria2QuicProfile {
    pub fn from_parts(client_fingerprint: Option<&str>) -> Self {
        Self {
            client_fingerprint: client_fingerprint.map(ToOwned::to_owned),
        }
    }

    fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }
}

#[cfg(feature = "hysteria2")]
impl<'a> Hysteria2ManagedUdpFlowConfig<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        client_fingerprint: Option<&'a str>,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            password,
            client_fingerprint,
        }
    }

    pub fn flow_resume(&self) -> Hysteria2ManagedDatagramFlowResume {
        Hysteria2ManagedDatagramFlowResume::new(
            hysteria2::udp::Hysteria2UdpFlowConfig::new(
                self.tag,
                self.server,
                self.port,
                self.password,
                self.client_fingerprint,
            )
            .flow_resume(),
        )
    }

    pub fn packet_path_carrier_descriptor(&self) -> Hysteria2ManagedUdpPacketPathCarrierDescriptor {
        Hysteria2ManagedUdpPacketPathCarrierDescriptor::new(
            hysteria2::udp::Hysteria2UdpFlowConfig::new(
                self.tag,
                self.server,
                self.port,
                self.password,
                self.client_fingerprint,
            )
            .packet_path_spec()
            .carrier_descriptor(self.server, self.port),
        )
    }

    pub fn packet_path_carrier_build(&self) -> Hysteria2ManagedUdpPacketPathCarrierBuild {
        Hysteria2ManagedUdpPacketPathCarrierBuild::new(
            hysteria2::udp::Hysteria2UdpFlowConfig::new(
                self.tag,
                self.server,
                self.port,
                self.password,
                self.client_fingerprint,
            )
            .packet_path_spec()
            .carrier_build(self.server, self.port),
        )
    }
}

#[cfg(feature = "hysteria2")]
impl<'a> Hysteria2TransportLeaf<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        password: &'a str,
        client_fingerprint: Option<&'a str>,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            password,
            client_fingerprint,
        }
    }

    pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self> {
        let ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
            ..
        } = leaf
        else {
            return None;
        };
        Some(Self::new(tag, server, *port, password, *client_fingerprint))
    }

    pub fn tag(&self) -> &str {
        self.tag
    }

    pub fn server(&self) -> &str {
        self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn flow_resume(&self) -> Hysteria2ManagedDatagramFlowResume {
        Hysteria2ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
        .flow_resume()
    }

    pub fn packet_path_carrier_descriptor(&self) -> Hysteria2ManagedUdpPacketPathCarrierDescriptor {
        Hysteria2ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
        .packet_path_carrier_descriptor()
    }

    pub fn packet_path_carrier_build(&self) -> Hysteria2ManagedUdpPacketPathCarrierBuild {
        Hysteria2ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
        .packet_path_carrier_build()
    }

    pub fn udp_flow_plan(&self) -> Hysteria2ManagedUdpFlowPlan<'a> {
        Hysteria2ManagedUdpFlowPlan::new(self.tag, self.server, self.port, self.flow_resume())
    }

    pub fn udp_packet_path_plan(&self) -> Hysteria2ManagedUdpPacketPathPlan {
        Hysteria2ManagedUdpPacketPathPlan::new(
            self.packet_path_carrier_descriptor(),
            self.packet_path_carrier_build(),
        )
    }

    pub async fn open_tcp_stream(
        &self,
        session: &Session,
    ) -> Result<crate::TcpRelayStream, EngineError> {
        connect_hysteria2_tcp_outbound(
            session,
            self.server,
            self.port,
            self.password,
            self.client_fingerprint,
        )
        .await
    }
}

#[cfg(feature = "hysteria2")]
pub fn udp_flow_resume_from_config(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Hysteria2ManagedDatagramFlowResume {
    Hysteria2ManagedUdpFlowConfig::new(tag, server, port, password, client_fingerprint)
        .flow_resume()
}

#[cfg(feature = "hysteria2")]
pub fn udp_packet_path_carrier_descriptor_from_config(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Hysteria2ManagedUdpPacketPathCarrierDescriptor {
    Hysteria2ManagedUdpFlowConfig::new(tag, server, port, password, client_fingerprint)
        .packet_path_carrier_descriptor()
}

#[cfg(feature = "hysteria2")]
pub fn udp_packet_path_carrier_build_from_config(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Hysteria2ManagedUdpPacketPathCarrierBuild {
    Hysteria2ManagedUdpFlowConfig::new(tag, server, port, password, client_fingerprint)
        .packet_path_carrier_build()
}

#[cfg(feature = "hysteria2")]
impl Hysteria2ManagedDatagramFlowResume {
    fn new(protocol: hysteria2::udp::Hysteria2UdpFlowResume) -> Self {
        Self { protocol }
    }

    fn connector_flow(&self, server: &str, port: u16) -> hysteria2::udp::Hysteria2UdpConnectorFlow {
        hysteria2::udp::connector_flow_from_resume(&self.protocol, server, port)
    }

    fn into_protocol_resume(self) -> hysteria2::udp::Hysteria2UdpFlowResume {
        self.protocol
    }
}

#[cfg(feature = "hysteria2")]
impl<'a> Hysteria2ManagedUdpFlowPlan<'a> {
    fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        resume: Hysteria2ManagedDatagramFlowResume,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            resume,
        }
    }

    pub fn tag(&self) -> &str {
        self.tag
    }

    pub fn server(&self) -> &str {
        self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn into_parts(self) -> (&'a str, &'a str, u16, Hysteria2ManagedDatagramFlowResume) {
        (self.tag, self.server, self.port, self.resume)
    }

    pub fn into_resume(self) -> Hysteria2ManagedDatagramFlowResume {
        self.resume
    }
}

#[cfg(feature = "hysteria2")]
impl Hysteria2ManagedUdpPacketPathPlan {
    fn new(
        carrier_descriptor: Hysteria2ManagedUdpPacketPathCarrierDescriptor,
        carrier_build: Hysteria2ManagedUdpPacketPathCarrierBuild,
    ) -> Self {
        Self {
            carrier_descriptor,
            carrier_build,
        }
    }

    pub fn into_carrier_descriptor(self) -> Hysteria2ManagedUdpPacketPathCarrierDescriptor {
        self.carrier_descriptor
    }

    pub fn into_carrier_build(self) -> Hysteria2ManagedUdpPacketPathCarrierBuild {
        self.carrier_build
    }
}

#[cfg(feature = "hysteria2")]
impl Hysteria2ManagedUdpPacketPathCarrierDescriptor {
    fn new(protocol: hysteria2::udp::Hysteria2UdpPacketPathCarrierDescriptor) -> Self {
        Self { protocol }
    }

    pub fn into_parts(self) -> (String, String, u16) {
        self.protocol.into_parts()
    }
}

#[cfg(feature = "hysteria2")]
impl Hysteria2ManagedUdpPacketPathCarrierBuild {
    fn new(protocol: hysteria2::udp::Hysteria2UdpPacketPathCarrierBuild) -> Self {
        Self { protocol }
    }

    fn into_protocol_build(self) -> hysteria2::udp::Hysteria2UdpPacketPathCarrierBuild {
        self.protocol
    }
}

#[cfg(feature = "hysteria2")]
impl crate::managed_udp::ProtocolManagedDatagramUdpResumeMetadata
    for Hysteria2ManagedDatagramFlowResume
{
    const ESTABLISH_STAGE: &'static str = "h2_establish";
    const MISMATCH_STAGE: &'static str = "udp_hysteria2_resume";
    const MISMATCH_MESSAGE: &'static str = "expected Hysteria2 UDP flow resume";
}

#[cfg(feature = "hysteria2")]
#[async_trait::async_trait]
impl crate::managed_udp::ProtocolManagedDatagramUdpResumeConnectionOps
    for Hysteria2ManagedDatagramFlowResume
{
    type RawConnection = hysteria2::udp::Hysteria2UdpFlowConnection;

    fn connector_flow_cache_key(&self, server: &str, port: u16) -> String {
        self.connector_flow(server, port).into_cache_key()
    }

    async fn open_protocol_connection(
        &self,
        server: &str,
        port: u16,
        target: &Address,
        target_port: u16,
        payload: &[u8],
    ) -> Result<Self::RawConnection, EngineError> {
        let connector_profile = self
            .connector_flow(server, port)
            .into_connection_parts()
            .into_profile();
        let conn = Arc::new(open_udp_profile_connection(server, port, connector_profile).await?);
        let resume = self.clone().into_protocol_resume();
        Ok(hysteria2::udp::start_udp_flow_with_initial_packet(
            conn,
            target,
            target_port,
            payload,
            resume,
        ))
    }
}

#[cfg(feature = "hysteria2")]
pub fn managed_datagram_connector_flow_from_resume(
    resume: &Hysteria2ManagedDatagramFlowResume,
    server: &str,
    port: u16,
) -> hysteria2::udp::Hysteria2UdpConnectorFlow {
    resume.connector_flow(server, port)
}

#[cfg(feature = "hysteria2")]
pub async fn accept_and_dispatch_authenticated_hysteria2_quic_session<
    Udp,
    UdpFut,
    Tcp,
    TcpFut,
    TaskResult,
    TaskResultFut,
    E,
>(
    profile: &OwnedHysteria2InboundProfile,
    conn: quinn::Connection,
    on_udp_session: Udp,
    on_tcp_stream: Tcp,
    on_stream_task_result: TaskResult,
) -> Result<(), E>
where
    Udp: FnMut(
        std::sync::Arc<quinn::Connection>,
        hysteria2::udp::Hysteria2InboundUdpRelay,
        &mut JoinSet<Result<(), E>>,
    ) -> UdpFut,
    UdpFut: Future<Output = Result<(), E>>,
    Tcp: FnMut(Session, Hysteria2Stream, &mut JoinSet<Result<(), E>>) -> TcpFut,
    TcpFut: Future<Output = Result<(), E>>,
    TaskResult: FnMut(Result<Result<(), E>, tokio::task::JoinError>) -> TaskResultFut,
    TaskResultFut: Future<Output = Result<(), E>>,
    E: From<zero_core::Error> + Send + 'static,
{
    profile
        .protocol
        .accept_and_dispatch_authenticated_quic_session(
            conn,
            Hysteria2Stream::new,
            on_udp_session,
            on_tcp_stream,
            on_stream_task_result,
        )
        .await
}

pub async fn open_quic_connection(
    options: QuicConnectionOptions<'_>,
) -> Result<quinn::Connection, EngineError> {
    let config_base = if let Some(fp_name) = options.quic_profile.client_fingerprint() {
        if let Some(preset) = crate::fingerprint::lookup_fingerprint(fp_name) {
            let provider = std::sync::Arc::new(crate::fingerprint::build_provider(&preset));
            tracing::debug!(
                fingerprint = %fp_name,
                "quic tls fingerprint applied"
            );
            rustls::ClientConfig::builder_with_provider(provider)
                .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
                .map_err(|error| {
                    EngineError::Io(io::Error::other(format!("quic tls protocol: {error}")))
                })?
        } else {
            tracing::warn!(
                fingerprint = %fp_name,
                "unknown quic tls fingerprint, using defaults"
            );
            rustls::ClientConfig::builder()
        }
    } else {
        rustls::ClientConfig::builder()
    };

    let mut tls_config = config_base
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(SkipVerify))
        .with_no_client_auth();
    tls_config.alpn_protocols = options.alpn;

    let quic_cfg = quinn::crypto::rustls::QuicClientConfig::try_from(tls_config)
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic tls cfg: {error}"))))?;

    let mut client_cfg = quinn::ClientConfig::new(Arc::new(quic_cfg));
    let mut transport = quinn::TransportConfig::default();
    transport.max_idle_timeout(Some(std::time::Duration::from_secs(30).try_into().unwrap()));
    transport.datagram_receive_buffer_size(options.datagram_receive_buffer_size);
    client_cfg.transport_config(Arc::new(transport));

    let bind_addr: std::net::SocketAddr = "0.0.0.0:0"
        .parse()
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic bind addr: {error}"))))?;
    let socket = std::net::UdpSocket::bind(bind_addr)
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic bind socket: {error}"))))?;
    let mut endpoint = quinn::Endpoint::new(
        quinn::EndpointConfig::default(),
        None,
        socket,
        Arc::new(quinn::TokioRuntime),
    )
    .map_err(|error| EngineError::Io(io::Error::other(format!("quic endpoint: {error}"))))?;
    endpoint.set_default_client_config(client_cfg);

    let server_addr = format!("{}:{}", options.server, options.port)
        .parse::<std::net::SocketAddr>()
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic addr: {error}"))))?;

    endpoint
        .connect(server_addr, options.server)
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic connect: {error}"))))?
        .await
        .map_err(|error| EngineError::Io(io::Error::other(format!("quic connection: {error}"))))
}

#[cfg(feature = "hysteria2")]
async fn open_authenticated_hysteria2_quic_connection(
    server: &str,
    port: u16,
    profile: &hysteria2::Hysteria2OutboundProfile,
) -> Result<quinn::Connection, EngineError> {
    let quic_profile = Hysteria2QuicProfile::from_parts(profile.client_fingerprint());
    let conn = open_quic_connection(QuicConnectionOptions {
        server,
        port,
        alpn: vec![b"hysteria2".to_vec()],
        quic_profile,
        datagram_receive_buffer_size: Some(65536),
    })
    .await?;

    let (send, recv) = conn.open_bi().await.map_err(|error| {
        EngineError::Io(io::Error::other(format!("hysteria2 open_bi: {error}")))
    })?;
    let mut stream = Hysteria2Stream::new(send, recv);
    profile
        .authenticate_connection(&conn, &mut stream)
        .await
        .map_err(EngineError::Core)?;

    Ok(conn)
}

#[cfg(feature = "hysteria2")]
async fn open_udp_profile_connection(
    server: &str,
    port: u16,
    connector_profile: hysteria2::udp::Hysteria2UdpConnectorProfile,
) -> Result<quinn::Connection, EngineError> {
    let quic_profile = Hysteria2QuicProfile::from_parts(connector_profile.client_fingerprint());
    let conn = open_quic_connection(QuicConnectionOptions {
        server,
        port,
        alpn: vec![b"hysteria2".to_vec()],
        quic_profile,
        datagram_receive_buffer_size: Some(65536),
    })
    .await?;

    let (send, recv) = conn.open_bi().await.map_err(|error| {
        EngineError::Io(io::Error::other(format!("hysteria2 open_bi: {error}")))
    })?;
    let mut stream = Hysteria2Stream::new(send, recv);
    connector_profile
        .authenticate_connection(&conn, &mut stream)
        .await
        .map_err(EngineError::Core)?;

    Ok(conn)
}

#[cfg(feature = "hysteria2")]
pub async fn connect_hysteria2_tcp_outbound(
    session: &Session,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> Result<crate::TcpRelayStream, EngineError> {
    let profile = hysteria2::outbound_profile_from_config_password(password, client_fingerprint);
    let conn = open_authenticated_hysteria2_quic_connection(server, port, &profile).await?;
    let (send, recv) = conn.open_bi().await.map_err(|error| {
        EngineError::Io(io::Error::other(format!("hysteria2 open_bi: {error}")))
    })?;
    let mut stream = Hysteria2Stream::new(send, recv);
    hysteria2::Hysteria2Outbound
        .establish_tcp_connect(&mut stream, session)
        .await
        .map_err(EngineError::Core)?;
    Ok(crate::TcpRelayStream::new(stream))
}

#[cfg(feature = "hysteria2")]
pub async fn open_hysteria2_udp_packet_path_build(
    build: Hysteria2ManagedUdpPacketPathCarrierBuild,
) -> Result<
    (
        quinn::Connection,
        Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    ),
    EngineError,
> {
    let parts = build.into_protocol_build().into_connection_parts();
    let (server, port, connector_profile, codec) = parts.into_shared_codec_parts();
    let conn = open_udp_profile_connection(&server, port, connector_profile).await?;
    Ok((conn, codec))
}

#[cfg(feature = "hysteria2")]
pub async fn establish_hysteria2_udp_flow_connection(
    server: &str,
    port: u16,
    target: &Address,
    target_port: u16,
    payload: &[u8],
    resume: Hysteria2ManagedDatagramFlowResume,
) -> Result<hysteria2::udp::Hysteria2UdpFlowConnection, EngineError> {
    let flow = managed_datagram_connector_flow_from_resume(&resume, server, port);
    let connector_profile = flow.into_connection_parts().into_profile();
    let conn = Arc::new(open_udp_profile_connection(server, port, connector_profile).await?);
    let resume = resume.into_protocol_resume();
    Ok(hysteria2::udp::start_udp_flow_with_initial_packet(
        conn,
        target,
        target_port,
        payload,
        resume,
    ))
}

#[cfg(feature = "hysteria2")]
#[async_trait::async_trait]
impl ManagedTupleUdpConnectionOps for hysteria2::udp::Hysteria2UdpFlowConnection {
    type SendError = zero_core::Error;

    async fn send_protocol_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Self::SendError> {
        hysteria2::udp::Hysteria2UdpFlowConnection::send(self, target, port, payload).await
    }

    fn subscribe_protocol_packets(&self) -> hysteria2::udp::Hysteria2UdpFlowResponseReceiver {
        hysteria2::udp::Hysteria2UdpFlowConnection::subscribe_responses(self)
    }

    fn closed_message_for_connection(&self) -> &'static str {
        "h2 upstream closed"
    }
}

#[derive(Debug)]
struct SkipVerify;

impl rustls::client::danger::ServerCertVerifier for SkipVerify {
    fn verify_server_cert(
        &self,
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &[rustls::pki_types::CertificateDer<'_>],
        _: &rustls::pki_types::ServerName<'_>,
        _: &[u8],
        _: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _: &[u8],
        _: &rustls::pki_types::CertificateDer<'_>,
        _: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ED25519,
        ]
    }
}
