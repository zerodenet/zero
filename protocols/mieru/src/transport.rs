//! Mieru transport-owned UDP bridge helpers.

mod managed_udp;
mod options;

use core::future::Future;

use tokio::io::{AsyncRead, AsyncWrite};
use zero_core::{InboundClientResponse, Session};
use zero_platform_tokio::{TcpRelayStream, TokioSocket};
use zero_traits::AsyncSocket;
use zero_transport::managed_udp::ManagedStreamPacketBridgePlan;
use zero_transport::outbound_leaf::{ProtocolSocketTcpHandshake, ProtocolTransportLeaf};
use zero_transport::RuntimeError;
use zero_transport::StreamTraffic;

pub use managed_udp::{MieruManagedStreamUdpResume, MieruManagedUdpFlowConfig};
pub use options::{MieruInboundUserRef, MieruOutboundOptionsRef};

#[derive(Debug, Clone)]
pub struct MieruInboundListenerRequest {
    protocol: crate::inbound::MieruInboundProfile,
}

#[derive(Debug, Default, Clone)]
pub struct MieruInboundResponseProtocol {
    protocol: crate::inbound::MieruInbound,
}

#[derive(Debug, Clone)]
pub struct MieruTransportLeaf {
    tag: String,
    server: String,
    port: u16,
    username: String,
    password: String,
}

impl ProtocolTransportLeaf for MieruTransportLeaf {
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
impl ProtocolSocketTcpHandshake for MieruTransportLeaf {
    fn connect_stage(&self) -> &'static str {
        "connect_upstream_mieru"
    }

    async fn handshake_socket(
        &self,
        socket: TokioSocket,
        session: &Session,
    ) -> Result<(TcpRelayStream, StreamTraffic), RuntimeError> {
        let stream = establish_mieru_tcp_tunnel(
            TcpRelayStream::new(socket),
            session,
            &self.username,
            &self.password,
        )
        .await?;
        Ok((stream, StreamTraffic::default()))
    }

    async fn handshake_relay(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError> {
        establish_mieru_tcp_tunnel(stream, session, &self.username, &self.password).await
    }
}

#[derive(Debug, Clone)]
pub struct MieruManagedUdpFlowPlan {
    tag: String,
    server: String,
    port: u16,
    resume: MieruManagedStreamUdpResume,
    relay_chain: bool,
}

impl MieruInboundListenerRequest {
    pub fn from_options_refs<'a, I>(users: I) -> Self
    where
        I: IntoIterator<Item = MieruInboundUserRef<'a>>,
    {
        Self::new(crate::inbound::inbound_profile_from_config_users(
            users.into_iter().map(|user| (user.username, user.password)),
        ))
    }

    fn new(protocol: crate::inbound::MieruInboundProfile) -> Self {
        Self { protocol }
    }

    pub fn response_protocol(&self) -> MieruInboundResponseProtocol {
        MieruInboundResponseProtocol::default()
    }

    pub async fn accept_and_dispatch_client<S, Tcp, TcpFut, Udp, UdpFut, E>(
        &self,
        stream: S,
        tcp: Tcp,
        udp: Udp,
    ) -> Result<(), E>
    where
        S: AsyncSocket + AsyncRead + AsyncWrite + Unpin,
        Tcp: FnOnce(Session, crate::inbound::MieruInboundStream<S>) -> TcpFut,
        TcpFut: core::future::Future<Output = Result<(), E>>,
        Udp: FnOnce(
            Session,
            crate::inbound::MieruInboundUdpRelay<crate::inbound::MieruInboundStream<S>>,
        ) -> UdpFut,
        UdpFut: core::future::Future<Output = Result<(), E>>,
        E: From<zero_core::Error>,
    {
        self.protocol
            .accept_and_dispatch_client(stream, tcp, udp)
            .await
    }
}

impl<S> InboundClientResponse<crate::inbound::MieruInboundStream<S>>
    for MieruInboundResponseProtocol
where
    S: AsyncRead + AsyncWrite + Unpin + Send + Sync,
{
    async fn send_ok(
        &self,
        client: &mut crate::inbound::MieruInboundStream<S>,
    ) -> Result<(), zero_core::Error> {
        self.protocol.send_ok(client).await
    }

    async fn send_blocked(
        &self,
        client: &mut crate::inbound::MieruInboundStream<S>,
    ) -> Result<(), zero_core::Error> {
        self.protocol.send_blocked(client).await
    }

    async fn send_upstream_failure(
        &self,
        client: &mut crate::inbound::MieruInboundStream<S>,
    ) -> Result<(), zero_core::Error> {
        self.protocol.send_upstream_failure(client).await
    }
}

impl MieruTransportLeaf {
    pub fn from_options_refs(
        tag: &str,
        server: &str,
        port: u16,
        options: MieruOutboundOptionsRef<'_>,
    ) -> Self {
        Self::new(tag, server, port, options.username, options.password)
    }

    pub fn new(tag: &str, server: &str, port: u16, username: &str, password: &str) -> Self {
        Self {
            tag: tag.to_owned(),
            server: server.to_owned(),
            port,
            username: username.to_owned(),
            password: password.to_owned(),
        }
    }

    pub fn tag(&self) -> &str {
        &self.tag
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn flow_resume(&self, relay_chain: bool) -> MieruManagedStreamUdpResume {
        MieruManagedUdpFlowConfig::new(&self.server, self.port, &self.username, &self.password)
            .flow_resume(relay_chain)
    }

    pub fn udp_flow_plan(&self, relay_chain: bool) -> MieruManagedUdpFlowPlan {
        MieruManagedUdpFlowPlan::new(
            self.tag.clone(),
            self.server.clone(),
            self.port,
            self.flow_resume(relay_chain),
            relay_chain,
        )
    }

    pub async fn open_tcp_stream<OpenSocket, OpenSocketFut, E>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<TcpRelayStream, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>> + Send,
        E: Into<RuntimeError>,
    {
        let socket = open_socket(&self.server, self.port)
            .await
            .map_err(Into::into)?;
        establish_mieru_tcp_tunnel(
            TcpRelayStream::new(socket),
            session,
            &self.username,
            &self.password,
        )
        .await
    }

    pub async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError> {
        establish_mieru_tcp_tunnel(stream, session, &self.username, &self.password).await
    }
}

pub async fn establish_mieru_tcp_tunnel(
    stream: TcpRelayStream,
    session: &Session,
    username: &str,
    password: &str,
) -> Result<TcpRelayStream, RuntimeError> {
    let mieru_stream = crate::tcp_outbound_profile_from_config(username, password)
        .establish_tcp_tunnel(stream, session)
        .await
        .map_err(|error| {
            RuntimeError::Io(std::io::Error::other(format!("mieru tcp tunnel: {error}")))
        })?;
    Ok(TcpRelayStream::new(mieru_stream))
}

impl MieruManagedUdpFlowPlan {
    fn new(
        tag: String,
        server: String,
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
        &self.tag
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn into_parts(self) -> (String, String, u16, MieruManagedStreamUdpResume) {
        (self.tag, self.server, self.port, self.resume)
    }

    pub fn into_bridge_plan(self) -> ManagedStreamPacketBridgePlan<MieruManagedStreamUdpResume> {
        ManagedStreamPacketBridgePlan::new(
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
