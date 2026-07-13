//! Mieru transport-owned UDP bridge helpers.

mod managed_udp;

use core::future::Future;

use crate::outbound_leaf::{ProtocolSocketTcpHandshake, ProtocolTransportLeaf};
use crate::StreamTraffic;
use tokio::io::{AsyncRead, AsyncWrite};
use zero_config::{InboundProtocolConfig, MieruUserConfig};
use zero_core::{InboundClientResponse, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TokioSocket;
use zero_traits::AsyncSocket;

pub use managed_udp::{MieruManagedStreamUdpResume, MieruManagedUdpFlowConfig};
pub use zero_platform_tokio::TcpRelayStream;

#[derive(Debug, Clone)]
pub struct OwnedMieruInboundProfile {
    protocol: mieru::inbound::MieruInboundProfile,
}

#[derive(Debug, Default, Clone)]
pub struct OwnedMieruInboundResponseProtocol {
    protocol: mieru::inbound::MieruInbound,
}

#[derive(Debug, Clone, Copy)]
pub struct MieruTransportLeaf<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    username: &'a str,
    password: &'a str,
}

impl ProtocolTransportLeaf for MieruTransportLeaf<'_> {
    fn tag(&self) -> &str {
        self.tag()
    }
    fn server(&self) -> &str {
        self.server()
    }
    fn port(&self) -> u16 {
        self.port()
    }
}

#[async_trait::async_trait]
impl ProtocolSocketTcpHandshake for MieruTransportLeaf<'_> {
    fn connect_stage(&self) -> &'static str {
        "connect_upstream_mieru"
    }

    async fn handshake_socket(
        &self,
        socket: TokioSocket,
        session: &Session,
    ) -> Result<(TcpRelayStream, StreamTraffic), EngineError> {
        let stream = establish_mieru_tcp_tunnel(
            TcpRelayStream::new(socket),
            session,
            self.username,
            self.password,
        )
        .await?;
        Ok((stream, StreamTraffic::default()))
    }

    async fn handshake_relay(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, EngineError> {
        establish_mieru_tcp_tunnel(stream, session, self.username, self.password).await
    }
}

#[derive(Debug, Clone)]
pub struct MieruManagedUdpFlowPlan<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    resume: MieruManagedStreamUdpResume,
    relay_chain: bool,
}

pub fn inbound_profile_from_users(users: &[MieruUserConfig]) -> OwnedMieruInboundProfile {
    OwnedMieruInboundProfile::new(mieru::inbound::inbound_profile_from_config_users(
        users
            .iter()
            .map(|user| (user.username.as_str(), user.password.as_str())),
    ))
}

pub fn inbound_profile_from_protocol(
    protocol: &InboundProtocolConfig,
) -> Result<OwnedMieruInboundProfile, EngineError> {
    match protocol {
        InboundProtocolConfig::Mieru { users } => Ok(inbound_profile_from_users(users)),
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "mieru inbound profile received non-mieru inbound config",
        ))),
    }
}

impl OwnedMieruInboundProfile {
    fn new(protocol: mieru::inbound::MieruInboundProfile) -> Self {
        Self { protocol }
    }

    pub fn response_protocol(&self) -> OwnedMieruInboundResponseProtocol {
        OwnedMieruInboundResponseProtocol::default()
    }

    pub async fn accept_and_dispatch_client<S, Tcp, TcpFut, Udp, UdpFut, E>(
        &self,
        stream: S,
        tcp: Tcp,
        udp: Udp,
    ) -> Result<(), E>
    where
        S: AsyncSocket + AsyncRead + AsyncWrite + Unpin,
        Tcp: FnOnce(Session, mieru::inbound::MieruInboundStream<S>) -> TcpFut,
        TcpFut: core::future::Future<Output = Result<(), E>>,
        Udp: FnOnce(
            Session,
            mieru::inbound::MieruInboundUdpRelay<mieru::inbound::MieruInboundStream<S>>,
        ) -> UdpFut,
        UdpFut: core::future::Future<Output = Result<(), E>>,
        E: From<zero_core::Error>,
    {
        self.protocol
            .accept_and_dispatch_client(stream, tcp, udp)
            .await
    }
}

impl<S> InboundClientResponse<mieru::inbound::MieruInboundStream<S>>
    for OwnedMieruInboundResponseProtocol
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    async fn send_ok(
        &self,
        client: &mut mieru::inbound::MieruInboundStream<S>,
    ) -> Result<(), zero_core::Error> {
        self.protocol.send_ok(client).await
    }

    async fn send_blocked(
        &self,
        client: &mut mieru::inbound::MieruInboundStream<S>,
    ) -> Result<(), zero_core::Error> {
        self.protocol.send_blocked(client).await
    }

    async fn send_upstream_failure(
        &self,
        client: &mut mieru::inbound::MieruInboundStream<S>,
    ) -> Result<(), zero_core::Error> {
        self.protocol.send_upstream_failure(client).await
    }
}

impl<'a> MieruTransportLeaf<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        username: &'a str,
        password: &'a str,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            username,
            password,
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

    pub fn flow_resume(&self, relay_chain: bool) -> MieruManagedStreamUdpResume {
        MieruManagedUdpFlowConfig::new(self.server, self.port, self.username, self.password)
            .flow_resume(relay_chain)
    }

    pub fn udp_flow_plan(&self, relay_chain: bool) -> MieruManagedUdpFlowPlan<'a> {
        MieruManagedUdpFlowPlan::new(
            self.tag,
            self.server,
            self.port,
            self.flow_resume(relay_chain),
            relay_chain,
        )
    }

    pub async fn open_tcp_stream<OpenSocket, OpenSocketFut, E>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<TcpRelayStream, EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>> + Send,
        E: Into<EngineError>,
    {
        let socket = open_socket(self.server, self.port)
            .await
            .map_err(Into::into)?;
        establish_mieru_tcp_tunnel(
            TcpRelayStream::new(socket),
            session,
            self.username,
            self.password,
        )
        .await
    }

    pub async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, EngineError> {
        establish_mieru_tcp_tunnel(stream, session, self.username, self.password).await
    }
}

pub async fn establish_mieru_tcp_tunnel(
    stream: TcpRelayStream,
    session: &Session,
    username: &str,
    password: &str,
) -> Result<TcpRelayStream, EngineError> {
    let mieru_stream = mieru::tcp_outbound_profile_from_config(username, password)
        .establish_tcp_tunnel(stream, session)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!("mieru tcp tunnel: {error}")))
        })?;
    Ok(TcpRelayStream::new(mieru_stream))
}

pub fn udp_flow_resume_from_config(
    server: &str,
    port: u16,
    username: &str,
    password: &str,
    relay_chain: bool,
) -> MieruManagedStreamUdpResume {
    MieruManagedUdpFlowConfig::new(server, port, username, password).flow_resume(relay_chain)
}

impl<'a> MieruManagedUdpFlowPlan<'a> {
    fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        resume: MieruManagedStreamUdpResume,
        relay_chain: bool,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            resume,
            relay_chain,
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

    pub fn into_parts(self) -> (&'a str, &'a str, u16, MieruManagedStreamUdpResume) {
        (self.tag, self.server, self.port, self.resume)
    }

    pub fn into_bridge_plan(
        self,
    ) -> crate::managed_udp::ManagedStreamPacketBridgePlan<'a, MieruManagedStreamUdpResume> {
        crate::managed_udp::ManagedStreamPacketBridgePlan::new(
            self.tag,
            self.server,
            self.port,
            self.resume,
            self.relay_chain,
        )
    }

    pub fn into_resume(self) -> MieruManagedStreamUdpResume {
        self.resume
    }
}
