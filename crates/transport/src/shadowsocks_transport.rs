//! Shadowsocks UDP socket flow transport helpers.

use core::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::{debug, warn};
use zero_config::InboundProtocolConfig;
use zero_core::{Address, Session, UdpFlowPacket};
use zero_engine::EngineError;
use zero_engine::ResolvedLeafOutbound;
use zero_platform_tokio::TokioSocket;
use zero_traits::{AsyncSocket, DatagramCodec};

use crate::managed_udp::ManagedDatagramConnectionOps;
use crate::{MeteredStream, StreamTraffic, TcpRelayStream};

pub type ShadowsocksUdpResponse = (Address, u16, Vec<u8>);

#[derive(Debug, Clone)]
pub struct ShadowsocksManagedDatagramFlowResume {
    protocol: shadowsocks::udp::ShadowsocksUdpFlowResume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowsocksManagedUdpPacketPathCarrierDescriptor {
    protocol: shadowsocks::udp::ShadowsocksUdpPacketPathCarrierDescriptor,
}

#[derive(Debug, Clone)]
pub struct ShadowsocksManagedUdpPacketPathDatagramSourceBuild {
    protocol: shadowsocks::udp::ShadowsocksUdpPacketPathDatagramSourceBuild,
}

#[derive(Debug, Clone)]
pub struct ShadowsocksManagedUdpFlowPlan<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    resume: ShadowsocksManagedDatagramFlowResume,
}

#[derive(Clone)]
pub struct ShadowsocksManagedUdpPacketPathPlan<'a> {
    server: &'a str,
    port: u16,
    carrier_descriptor: ShadowsocksManagedUdpPacketPathCarrierDescriptor,
    carrier_codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    datagram_source: ShadowsocksManagedUdpPacketPathDatagramSourceBuild,
}

#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksManagedUdpFlowConfig<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    cipher: &'a str,
    password: &'a str,
}

#[derive(Debug, Clone, Copy)]
pub struct ShadowsocksTransportLeaf<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    cipher: &'a str,
    password: &'a str,
}

pub struct ShadowsocksUdpSocketFlow {
    socket: Arc<tokio::net::UdpSocket>,
    endpoint: SocketAddr,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    recv_tx: broadcast::Sender<ShadowsocksUdpResponse>,
}

#[derive(Debug, Clone)]
pub struct OwnedShadowsocksInboundProfile {
    protocol: shadowsocks::ShadowsocksInboundProfile,
}

#[derive(Clone)]
pub struct OwnedShadowsocksInboundTcpAcceptor {
    protocol: shadowsocks::ShadowsocksInboundTcpAcceptor,
}

pub struct OwnedShadowsocksInboundBindings {
    acceptor: OwnedShadowsocksInboundTcpAcceptor,
    udp_relay: shadowsocks::udp::ShadowsocksInboundUdpRelay,
}

pub fn inbound_profile_from_protocol(
    protocol: &InboundProtocolConfig,
) -> Result<OwnedShadowsocksInboundProfile, EngineError> {
    match protocol {
        InboundProtocolConfig::Shadowsocks {
            password, cipher, ..
        } => shadowsocks::inbound_profile_from_config_cipher_password(
            cipher.as_str(),
            password.as_str(),
        )
        .map(OwnedShadowsocksInboundProfile::new)
        .map_err(|error| {
            EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid shadowsocks inbound profile: {error}"),
            ))
        }),
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "shadowsocks inbound profile received non-shadowsocks inbound config",
        ))),
    }
}

impl OwnedShadowsocksInboundProfile {
    fn new(protocol: shadowsocks::ShadowsocksInboundProfile) -> Self {
        Self { protocol }
    }

    pub fn into_listener_bindings(self) -> OwnedShadowsocksInboundBindings {
        let (acceptor, udp_relay) = self.protocol.into_listener_bindings();
        OwnedShadowsocksInboundBindings {
            acceptor: OwnedShadowsocksInboundTcpAcceptor::new(acceptor),
            udp_relay,
        }
    }
}

impl OwnedShadowsocksInboundTcpAcceptor {
    fn new(protocol: shadowsocks::ShadowsocksInboundTcpAcceptor) -> Self {
        Self { protocol }
    }

    pub async fn accept_and_dispatch_stream<S, H, HFut, E>(
        &self,
        stream: S,
        handoff: H,
    ) -> Result<(), E>
    where
        S: AsyncSocket,
        H: FnOnce(Session, shadowsocks::ShadowsocksAeadStream<S>) -> HFut,
        HFut: Future<Output = Result<(), E>>,
        E: From<zero_core::Error>,
    {
        self.protocol
            .accept_and_dispatch_stream(stream, handoff)
            .await
    }
}

impl OwnedShadowsocksInboundBindings {
    pub fn into_parts(
        self,
    ) -> (
        OwnedShadowsocksInboundTcpAcceptor,
        shadowsocks::udp::ShadowsocksInboundUdpRelay,
    ) {
        (self.acceptor, self.udp_relay)
    }
}

impl<'a> ShadowsocksManagedUdpFlowConfig<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        cipher: &'a str,
        password: &'a str,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            cipher,
            password,
        }
    }

    pub fn flow_resume(&self) -> Result<ShadowsocksManagedDatagramFlowResume, zero_core::Error> {
        self.protocol_config()
            .flow_resume()
            .map(ShadowsocksManagedDatagramFlowResume::new)
    }

    pub fn packet_path_carrier_descriptor(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathCarrierDescriptor, zero_core::Error> {
        Ok(ShadowsocksManagedUdpPacketPathCarrierDescriptor::new(
            self.protocol_config()
                .packet_path_spec()?
                .carrier_descriptor(self.server, self.port),
        ))
    }

    pub fn packet_path_carrier_codec(
        &self,
    ) -> Result<Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>, zero_core::Error> {
        Ok(self.protocol_config().packet_path_spec()?.carrier_codec())
    }

    pub fn packet_path_datagram_source_build(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathDatagramSourceBuild, zero_core::Error> {
        Ok(ShadowsocksManagedUdpPacketPathDatagramSourceBuild::new(
            self.protocol_config()
                .packet_path_spec()?
                .datagram_source_build(self.tag, self.server, self.port),
        ))
    }

    fn protocol_config(&self) -> shadowsocks::udp::ShadowsocksUdpFlowConfig<'a> {
        shadowsocks::udp::ShadowsocksUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.cipher,
            self.password,
        )
    }
}

impl<'a> ShadowsocksTransportLeaf<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        cipher: &'a str,
        password: &'a str,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            cipher,
            password,
        }
    }

    pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self> {
        let ResolvedLeafOutbound::Shadowsocks {
            tag,
            server,
            port,
            password,
            cipher,
        } = leaf
        else {
            return None;
        };
        Some(Self::new(tag, server, *port, cipher, password))
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

    pub fn cipher(&self) -> &str {
        self.cipher
    }

    pub fn password(&self) -> &str {
        self.password
    }

    pub fn flow_resume(&self) -> Result<ShadowsocksManagedDatagramFlowResume, zero_core::Error> {
        ShadowsocksManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.cipher,
            self.password,
        )
        .flow_resume()
    }

    pub fn packet_path_carrier_descriptor(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathCarrierDescriptor, zero_core::Error> {
        ShadowsocksManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.cipher,
            self.password,
        )
        .packet_path_carrier_descriptor()
    }

    pub fn packet_path_carrier_codec(
        &self,
    ) -> Result<Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>, zero_core::Error> {
        ShadowsocksManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.cipher,
            self.password,
        )
        .packet_path_carrier_codec()
    }

    pub fn packet_path_datagram_source_build(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathDatagramSourceBuild, zero_core::Error> {
        ShadowsocksManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.cipher,
            self.password,
        )
        .packet_path_datagram_source_build()
    }

    pub fn udp_flow_plan(&self) -> Result<ShadowsocksManagedUdpFlowPlan<'a>, zero_core::Error> {
        Ok(ShadowsocksManagedUdpFlowPlan::new(
            self.tag,
            self.server,
            self.port,
            self.flow_resume()?,
        ))
    }

    pub fn udp_packet_path_plan(
        &self,
    ) -> Result<ShadowsocksManagedUdpPacketPathPlan<'a>, zero_core::Error> {
        Ok(ShadowsocksManagedUdpPacketPathPlan::new(
            self.server,
            self.port,
            self.packet_path_carrier_descriptor()?,
            self.packet_path_carrier_codec()?,
            self.packet_path_datagram_source_build()?,
        ))
    }

    pub async fn open_tcp_stream<OpenSocket, OpenSocketFut, E>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<(TcpRelayStream, StreamTraffic), EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>> + Send,
        E: Into<EngineError>,
    {
        let upstream = open_socket(self.server, self.port)
            .await
            .map_err(Into::into)?;
        let metered = MeteredStream::new(TcpRelayStream::from(upstream));
        establish_shadowsocks_tcp_connect(metered, session, self.cipher, self.password).await
    }

    pub async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, EngineError> {
        apply_shadowsocks_tcp_relay_hop(stream, session, self.cipher, self.password).await
    }
}

pub async fn establish_shadowsocks_tcp_connect(
    mut stream: MeteredStream<TcpRelayStream>,
    session: &Session,
    cipher: &str,
    password: &str,
) -> Result<(TcpRelayStream, StreamTraffic), EngineError> {
    let config = shadowsocks_tcp_connect_config(cipher, password)?;
    let ss_session = config
        .establish_tcp_session(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    let traffic = stream.drain_traffic();
    let stream = stream.into_inner();
    Ok((
        TcpRelayStream::new(config.wrap_outbound_stream(stream, ss_session)),
        traffic,
    ))
}

pub async fn apply_shadowsocks_tcp_relay_hop(
    mut stream: TcpRelayStream,
    session: &Session,
    cipher: &str,
    password: &str,
) -> Result<TcpRelayStream, EngineError> {
    let config = shadowsocks_tcp_connect_config(cipher, password)?;
    let ss_session = config
        .establish_tcp_session(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    Ok(TcpRelayStream::new(
        config.wrap_outbound_stream(stream, ss_session),
    ))
}

pub fn udp_flow_resume_from_config(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> Result<ShadowsocksManagedDatagramFlowResume, zero_core::Error> {
    ShadowsocksManagedUdpFlowConfig::new(tag, server, port, cipher, password).flow_resume()
}

pub fn udp_packet_path_carrier_descriptor_from_config(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> Result<ShadowsocksManagedUdpPacketPathCarrierDescriptor, zero_core::Error> {
    ShadowsocksManagedUdpFlowConfig::new(tag, server, port, cipher, password)
        .packet_path_carrier_descriptor()
}

pub fn udp_packet_path_carrier_codec_from_config(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> Result<Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>, zero_core::Error> {
    ShadowsocksManagedUdpFlowConfig::new(tag, server, port, cipher, password)
        .packet_path_carrier_codec()
}

pub fn udp_packet_path_datagram_source_build_from_config(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> Result<ShadowsocksManagedUdpPacketPathDatagramSourceBuild, zero_core::Error> {
    ShadowsocksManagedUdpFlowConfig::new(tag, server, port, cipher, password)
        .packet_path_datagram_source_build()
}

fn shadowsocks_tcp_connect_config(
    cipher: &str,
    password: &str,
) -> Result<shadowsocks::ShadowsocksTcpConnectConfig, EngineError> {
    shadowsocks::tcp_connect_config_from_config(cipher, password).map_err(|error| {
        EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("invalid shadowsocks tcp config: {error}"),
        ))
    })
}

impl ShadowsocksManagedDatagramFlowResume {
    fn new(protocol: shadowsocks::udp::ShadowsocksUdpFlowResume) -> Self {
        Self { protocol }
    }

    fn socket_flow_spec(&self) -> shadowsocks::udp::ShadowsocksUdpSocketFlowSpec {
        shadowsocks::udp::managed_socket_flow_from_resume(&self.protocol)
    }

    fn into_shared_managed_socket_flow_codec(
        self,
    ) -> Arc<dyn DatagramCodec<Address, Error = zero_core::Error>> {
        self.protocol.into_shared_managed_socket_flow_codec()
    }
}

impl<'a> ShadowsocksManagedUdpFlowPlan<'a> {
    fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        resume: ShadowsocksManagedDatagramFlowResume,
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

    pub fn into_parts(self) -> (&'a str, &'a str, u16, ShadowsocksManagedDatagramFlowResume) {
        (self.tag, self.server, self.port, self.resume)
    }

    pub fn into_resume(self) -> ShadowsocksManagedDatagramFlowResume {
        self.resume
    }
}

impl<'a> ShadowsocksManagedUdpPacketPathPlan<'a> {
    fn new(
        server: &'a str,
        port: u16,
        carrier_descriptor: ShadowsocksManagedUdpPacketPathCarrierDescriptor,
        carrier_codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
        datagram_source: ShadowsocksManagedUdpPacketPathDatagramSourceBuild,
    ) -> Self {
        Self {
            server,
            port,
            carrier_descriptor,
            carrier_codec,
            datagram_source,
        }
    }

    pub fn server(&self) -> &str {
        self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn carrier_codec(&self) -> Arc<dyn DatagramCodec<Address, Error = zero_core::Error>> {
        self.carrier_codec.clone()
    }

    pub fn into_carrier_descriptor(self) -> ShadowsocksManagedUdpPacketPathCarrierDescriptor {
        self.carrier_descriptor
    }

    pub fn into_datagram_source_build(self) -> ShadowsocksManagedUdpPacketPathDatagramSourceBuild {
        self.datagram_source
    }
}

impl ShadowsocksManagedUdpPacketPathCarrierDescriptor {
    fn new(protocol: shadowsocks::udp::ShadowsocksUdpPacketPathCarrierDescriptor) -> Self {
        Self { protocol }
    }

    pub fn into_parts(self) -> (String, String, u16) {
        self.protocol.into_parts()
    }
}

impl ShadowsocksManagedUdpPacketPathDatagramSourceBuild {
    fn new(protocol: shadowsocks::udp::ShadowsocksUdpPacketPathDatagramSourceBuild) -> Self {
        Self { protocol }
    }

    pub fn into_shared_codec_parts(
        self,
    ) -> (
        String,
        String,
        u16,
        String,
        Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    ) {
        self.protocol.into_shared_codec_parts()
    }
}

impl crate::managed_udp::ProtocolManagedDatagramUdpResumeMetadata
    for ShadowsocksManagedDatagramFlowResume
{
    const ESTABLISH_STAGE: &'static str = "ss_establish";
    const MISMATCH_STAGE: &'static str = "udp_shadowsocks_resume";
    const MISMATCH_MESSAGE: &'static str = "expected Shadowsocks UDP flow resume";
}

#[async_trait::async_trait]
impl crate::managed_udp::ProtocolManagedDatagramSocketUdpResumeConnectionOps
    for ShadowsocksManagedDatagramFlowResume
{
    type RawConnection = ShadowsocksUdpSocketFlow;

    const SEND_STAGE: &'static str = "ss_send";
    const RESOLVE_UPSTREAM_MESSAGE: &'static str = "failed to resolve shadowsocks udp upstream";

    fn connector_flow_cache_key(&self, _server: &str, _port: u16) -> String {
        self.socket_flow_spec().into_cache_key()
    }

    async fn open_protocol_connection(
        &self,
        endpoint: SocketAddr,
    ) -> Result<Self::RawConnection, EngineError> {
        establish_shadowsocks_udp_socket_flow(
            endpoint,
            self.clone().into_shared_managed_socket_flow_codec(),
        )
        .await
    }
}

pub fn managed_socket_flow_from_resume(
    resume: &ShadowsocksManagedDatagramFlowResume,
) -> shadowsocks::udp::ShadowsocksUdpSocketFlowSpec {
    resume.socket_flow_spec()
}

pub async fn establish_shadowsocks_udp_socket_flow(
    endpoint: SocketAddr,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) -> Result<ShadowsocksUdpSocketFlow, EngineError> {
    let socket = Arc::new(bind_for_endpoint(endpoint).await?);
    let (recv_tx, _) = broadcast::channel::<ShadowsocksUdpResponse>(32);
    spawn_recv_loop(socket.clone(), codec.clone(), recv_tx.clone());

    Ok(ShadowsocksUdpSocketFlow {
        socket,
        endpoint,
        codec,
        recv_tx,
    })
}

pub async fn establish_shadowsocks_udp_socket_flow_with_resume(
    endpoint: SocketAddr,
    resume: ShadowsocksManagedDatagramFlowResume,
) -> Result<ShadowsocksUdpSocketFlow, EngineError> {
    establish_shadowsocks_udp_socket_flow(endpoint, resume.into_shared_managed_socket_flow_codec())
        .await
}

impl ShadowsocksUdpSocketFlow {
    pub fn subscribe(&self) -> broadcast::Receiver<ShadowsocksUdpResponse> {
        self.recv_tx.subscribe()
    }

    pub async fn send_packet(&self, packet: UdpFlowPacket) -> Result<(), EngineError> {
        self.send_datagram(&packet.target, packet.port, &packet.payload)
            .await
    }

    pub async fn send_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let datagram = self.codec.encode(target, port, payload)?;
        self.socket.send_to(&datagram, self.endpoint).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ManagedDatagramConnectionOps for ShadowsocksUdpSocketFlow {
    type SendError = EngineError;

    async fn send_protocol_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), Self::SendError> {
        self.send_datagram(target, port, payload).await
    }

    fn subscribe_protocol_datagrams(&self) -> broadcast::Receiver<ShadowsocksUdpResponse> {
        self.subscribe()
    }

    fn closed_message_for_datagram_connection(&self) -> &'static str {
        "ss upstream closed"
    }
}

async fn bind_for_endpoint(endpoint: SocketAddr) -> Result<tokio::net::UdpSocket, std::io::Error> {
    let bind_addr = match endpoint {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    tokio::net::UdpSocket::bind(bind_addr).await
}

fn spawn_recv_loop(
    socket: Arc<tokio::net::UdpSocket>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    recv_tx: broadcast::Sender<ShadowsocksUdpResponse>,
) {
    tokio::spawn(recv_loop(socket, codec, recv_tx));
}

async fn recv_loop(
    socket: Arc<tokio::net::UdpSocket>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    recv_tx: broadcast::Sender<ShadowsocksUdpResponse>,
) {
    let mut buf = vec![0u8; 4096];
    loop {
        let (n, sender) = match socket.recv_from(&mut buf).await {
            Ok(r) => r,
            Err(error) => {
                warn!(error = %error, "shadowsocks udp recv loop stopped");
                break;
            }
        };
        let datagram = &buf[..n];
        let Some((target, port, payload)) = codec.decode(datagram) else {
            warn!(
                upstream = %sender,
                bytes = n,
                "failed to decode shadowsocks udp response"
            );
            continue;
        };
        debug!(
            upstream = %sender,
            target = ?target,
            port = port,
            bytes = payload.len(),
            "decoded shadowsocks udp response"
        );
        if recv_tx.send((target, port, payload)).is_err() {
            break;
        }
    }
}
