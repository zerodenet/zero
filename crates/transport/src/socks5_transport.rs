use std::future::Future;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};

use zero_config::{InboundProtocolConfig, Socks5UserConfig};
use zero_core::{
    Address, InboundClientResponse, InboundUdpAssociation, InboundUdpAssociationDispatcher,
    InboundUdpAssociationResponder, InboundUdpAssociationResponse, Session,
};
use zero_engine::EngineError;
use zero_engine::ResolvedLeafOutbound;
use zero_platform_tokio::{TokioDatagramSocket, TokioSocket};
use zero_traits::{AsyncSocket, SocketAddress};

use crate::{ClientStream, MeteredStream, StreamTraffic, TcpRelayStream};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Socks5UpstreamAssociationCloseReason {
    Closed,
    IdleTimeout,
    Dropped,
}

#[derive(Debug, Clone, Copy)]
pub struct Socks5ManagedUdpFlowConfig<'a> {
    protocol: socks5::udp::Socks5UdpFlowConfig<'a>,
}

#[derive(Clone)]
pub struct OwnedSocks5InboundAcceptor {
    protocol: socks5::Socks5InboundTcpAcceptor,
}

pub struct Socks5InboundUdpAssociationSetup {
    pub relay: TokioDatagramSocket,
    pub pending_control_traffic: StreamTraffic,
    pub handler: Socks5InboundUdpAssociationHandler,
}

pub struct Socks5InboundUdpAssociationHandler {
    protocol: socks5::udp::Socks5InboundUdpAssociationSession,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5ManagedUdpAssociationTarget {
    protocol: socks5::udp::Socks5UdpAssociationTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5ManagedUdpPacketPathCarrierBuild {
    protocol: socks5::udp::Socks5UdpPacketPathCarrierBuild,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5ManagedUdpPacketPathCarrierDescriptor {
    protocol: socks5::udp::Socks5UdpPacketPathCarrierDescriptor,
}

#[derive(Debug, Clone)]
pub struct Socks5ManagedUdpFlowPlan<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    association_target: Socks5ManagedUdpAssociationTarget,
}

#[derive(Debug, Clone)]
pub struct Socks5ManagedUdpPacketPathPlan {
    carrier_descriptor: Socks5ManagedUdpPacketPathCarrierDescriptor,
    carrier_build: Socks5ManagedUdpPacketPathCarrierBuild,
}

#[derive(Debug, Clone, Copy)]
pub struct Socks5TransportLeaf<'a> {
    tag: &'a str,
    server: &'a str,
    port: u16,
    username: Option<&'a str>,
    password: Option<&'a str>,
}

pub fn inbound_acceptor_from_users(users: &[Socks5UserConfig]) -> OwnedSocks5InboundAcceptor {
    OwnedSocks5InboundAcceptor::new(socks5::Socks5InboundTcpAcceptor::from_config_users(
        users.iter().map(|user| {
            (
                user.username.as_str(),
                user.password.as_str(),
                user.principal_key.as_deref(),
                user.up_bps,
                user.down_bps,
            )
        }),
    ))
}

pub fn inbound_acceptor_from_protocol(
    protocol: &InboundProtocolConfig,
) -> Result<OwnedSocks5InboundAcceptor, EngineError> {
    match protocol {
        InboundProtocolConfig::Socks5 { users } => Ok(inbound_acceptor_from_users(users)),
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "socks5 inbound acceptor received non-socks5 inbound config",
        ))),
    }
}

pub async fn setup_inbound_udp_association<S>(
    client: &mut MeteredStream<S>,
    request: socks5::udp::Socks5UdpAssociateRequest,
) -> Result<Socks5InboundUdpAssociationSetup, EngineError>
where
    S: crate::ClientStream,
{
    let control_local_addr = client.local_addr()?;
    let relay = TokioDatagramSocket::bind_addr(SocketAddr::new(control_local_addr.ip(), 0)).await?;
    let relay_addr = relay.local_addr()?;
    let relay_bind = match zero_platform_tokio::socket_addr_to_ip(relay_addr) {
        zero_traits::IpAddress::V4(ip) => Address::Ipv4(ip),
        zero_traits::IpAddress::V6(ip) => Address::Ipv6(ip),
    };

    socks5::Socks5Inbound
        .send_success_response_with_bound(client, &relay_bind, relay_addr.port())
        .await?;

    Ok(Socks5InboundUdpAssociationSetup {
        relay,
        pending_control_traffic: client.drain_traffic(),
        handler: Socks5InboundUdpAssociationHandler::new(
            socks5::Socks5Inbound.accept_udp_association(request),
        ),
    })
}

impl OwnedSocks5InboundAcceptor {
    fn new(protocol: socks5::Socks5InboundTcpAcceptor) -> Self {
        Self { protocol }
    }

    pub async fn accept_and_dispatch_command<S, Connect, ConnectFut, Udp, UdpFut, E>(
        &self,
        stream: MeteredStream<S>,
        on_connect: Connect,
        on_udp_associate: Udp,
    ) -> Result<(), E>
    where
        S: ClientStream,
        Connect: FnOnce(Session, S) -> ConnectFut,
        ConnectFut: Future<Output = Result<(), E>>,
        Udp: FnOnce(Socks5InboundUdpAssociationSetup, MeteredStream<S>) -> UdpFut,
        UdpFut: Future<Output = Result<(), E>>,
        E: From<zero_core::Error> + From<EngineError>,
    {
        self.protocol
            .accept_and_dispatch_command_with(
                stream,
                |session, stream| async move { on_connect(session, stream.into_inner()).await },
                |request, mut stream| async move {
                    let setup = setup_inbound_udp_association(&mut stream, request)
                        .await
                        .map_err(E::from)?;
                    on_udp_associate(setup, stream).await
                },
            )
            .await
    }
}

impl<S> InboundClientResponse<S> for OwnedSocks5InboundAcceptor
where
    S: AsyncSocket,
{
    async fn send_ok(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_success(client).await
    }

    async fn send_blocked(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_blocked(client).await
    }

    async fn send_upstream_failure(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_upstream_failure(client).await
    }
}

impl<'a> Socks5ManagedUdpFlowConfig<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        username: Option<&'a str>,
        password: Option<&'a str>,
    ) -> Self {
        Self {
            protocol: socks5::udp::Socks5UdpFlowConfig::new(tag, server, port, username, password),
        }
    }

    pub fn association_target(&self) -> Socks5ManagedUdpAssociationTarget {
        Socks5ManagedUdpAssociationTarget::new(self.protocol.association_target())
    }

    pub fn packet_path_carrier_descriptor(&self) -> Socks5ManagedUdpPacketPathCarrierDescriptor {
        Socks5ManagedUdpPacketPathCarrierDescriptor::new(
            self.protocol.packet_path_spec().carrier_descriptor(),
        )
    }

    pub fn packet_path_carrier_build(&self) -> Socks5ManagedUdpPacketPathCarrierBuild {
        Socks5ManagedUdpPacketPathCarrierBuild::new(
            self.protocol.packet_path_spec().carrier_build(),
        )
    }
}

impl Socks5ManagedUdpAssociationTarget {
    fn new(protocol: socks5::udp::Socks5UdpAssociationTarget) -> Self {
        Self { protocol }
    }

    pub fn outbound_tag(&self) -> &str {
        self.protocol.outbound_tag()
    }

    pub fn log_parts(&self) -> (&str, &str, u16) {
        self.protocol.log_parts()
    }

    fn into_protocol_target(self) -> socks5::udp::Socks5UdpAssociationTarget {
        self.protocol
    }
}

impl Socks5InboundUdpAssociationHandler {
    fn new(protocol: socks5::udp::Socks5InboundUdpAssociationSession) -> Self {
        Self { protocol }
    }
}

impl InboundUdpAssociation for Socks5InboundUdpAssociationHandler {
    async fn dispatch_datagram<D>(
        &mut self,
        sender: SocketAddress,
        packet: &[u8],
        dispatcher: &mut D,
    ) -> Result<(), D::Error>
    where
        D: InboundUdpAssociationDispatcher,
        D::Error: From<zero_core::Error>,
    {
        self.protocol
            .dispatch_datagram(sender, packet, dispatcher)
            .await
    }
}

impl InboundUdpAssociationResponder for Socks5InboundUdpAssociationHandler {
    fn build_response_for_target(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<InboundUdpAssociationResponse>, zero_core::Error> {
        self.protocol
            .build_response_for_target(target, port, payload)
    }

    fn build_peer_response(
        &self,
        sender: SocketAddress,
        payload: &[u8],
    ) -> Result<Option<InboundUdpAssociationResponse>, zero_core::Error> {
        self.protocol.build_peer_response(sender, payload)
    }
}

impl Socks5ManagedUdpPacketPathCarrierBuild {
    fn new(protocol: socks5::udp::Socks5UdpPacketPathCarrierBuild) -> Self {
        Self { protocol }
    }

    fn into_protocol_build(self) -> socks5::udp::Socks5UdpPacketPathCarrierBuild {
        self.protocol
    }
}

impl Socks5ManagedUdpPacketPathCarrierDescriptor {
    fn new(protocol: socks5::udp::Socks5UdpPacketPathCarrierDescriptor) -> Self {
        Self { protocol }
    }

    pub fn into_parts(self) -> (String, String, u16) {
        self.protocol.into_parts()
    }
}

impl<'a> Socks5ManagedUdpFlowPlan<'a> {
    fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        association_target: Socks5ManagedUdpAssociationTarget,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            association_target,
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

    pub fn into_parts(self) -> (&'a str, &'a str, u16, Socks5ManagedUdpAssociationTarget) {
        (self.tag, self.server, self.port, self.association_target)
    }

    pub fn into_association_target(self) -> Socks5ManagedUdpAssociationTarget {
        self.association_target
    }
}

impl Socks5ManagedUdpPacketPathPlan {
    fn new(
        carrier_descriptor: Socks5ManagedUdpPacketPathCarrierDescriptor,
        carrier_build: Socks5ManagedUdpPacketPathCarrierBuild,
    ) -> Self {
        Self {
            carrier_descriptor,
            carrier_build,
        }
    }

    pub fn into_carrier_descriptor(self) -> Socks5ManagedUdpPacketPathCarrierDescriptor {
        self.carrier_descriptor
    }

    pub fn into_carrier_build(self) -> Socks5ManagedUdpPacketPathCarrierBuild {
        self.carrier_build
    }
}

impl<'a> Socks5TransportLeaf<'a> {
    pub fn new(
        tag: &'a str,
        server: &'a str,
        port: u16,
        username: Option<&'a str>,
        password: Option<&'a str>,
    ) -> Self {
        Self {
            tag,
            server,
            port,
            username,
            password,
        }
    }

    pub fn from_resolved_leaf(leaf: &ResolvedLeafOutbound<'a>) -> Option<Self> {
        let ResolvedLeafOutbound::Socks5 {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return None;
        };
        Some(Self::new(tag, server, *port, *username, *password))
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

    pub fn association_target(&self) -> Socks5ManagedUdpAssociationTarget {
        Socks5ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.username,
            self.password,
        )
        .association_target()
    }

    pub fn packet_path_carrier_descriptor(&self) -> Socks5ManagedUdpPacketPathCarrierDescriptor {
        Socks5ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.username,
            self.password,
        )
        .packet_path_carrier_descriptor()
    }

    pub fn packet_path_carrier_build(&self) -> Socks5ManagedUdpPacketPathCarrierBuild {
        Socks5ManagedUdpFlowConfig::new(
            self.tag,
            self.server,
            self.port,
            self.username,
            self.password,
        )
        .packet_path_carrier_build()
    }

    pub fn udp_flow_plan(&self) -> Socks5ManagedUdpFlowPlan<'a> {
        Socks5ManagedUdpFlowPlan::new(self.tag, self.server, self.port, self.association_target())
    }

    pub fn udp_packet_path_plan(&self) -> Socks5ManagedUdpPacketPathPlan {
        Socks5ManagedUdpPacketPathPlan::new(
            self.packet_path_carrier_descriptor(),
            self.packet_path_carrier_build(),
        )
    }

    pub async fn open_tcp_stream<OpenSocket, OpenSocketFut, E>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<(TcpRelayStream, StreamTraffic), EngineError>
    where
        OpenSocket: FnOnce(&str, u16) -> OpenSocketFut,
        OpenSocketFut: Future<Output = Result<TokioSocket, E>>,
        E: Into<EngineError>,
    {
        let upstream = open_socket(self.server, self.port)
            .await
            .map_err(Into::into)?;
        let metered = MeteredStream::new(TcpRelayStream::from(upstream));
        establish_socks5_tcp_connect(metered, session, self.username, self.password).await
    }

    pub async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, EngineError> {
        apply_socks5_tcp_relay_hop(stream, session, self.username, self.password).await
    }
}

pub async fn establish_socks5_tcp_connect(
    mut stream: MeteredStream<TcpRelayStream>,
    session: &Session,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<(TcpRelayStream, StreamTraffic), EngineError> {
    socks5::Socks5TcpOutboundProfile::from_config_parts(username, password)
        .establish_tcp_tunnel(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    let traffic = stream.drain_traffic();
    Ok((stream.into_inner(), traffic))
}

pub async fn apply_socks5_tcp_relay_hop(
    mut stream: TcpRelayStream,
    session: &Session,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<TcpRelayStream, EngineError> {
    socks5::Socks5TcpOutboundProfile::from_config_parts(username, password)
        .establish_tcp_tunnel(&mut stream, session)
        .await
        .map_err(|error| EngineError::Io(std::io::Error::other(error)))?;
    Ok(stream)
}

pub fn udp_association_target_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5ManagedUdpAssociationTarget {
    Socks5ManagedUdpFlowConfig::new(tag, server, port, username, password).association_target()
}

pub fn udp_packet_path_carrier_descriptor_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5ManagedUdpPacketPathCarrierDescriptor {
    Socks5ManagedUdpFlowConfig::new(tag, server, port, username, password)
        .packet_path_carrier_descriptor()
}

pub fn udp_packet_path_carrier_build_from_config(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> Socks5ManagedUdpPacketPathCarrierBuild {
    Socks5ManagedUdpFlowConfig::new(tag, server, port, username, password)
        .packet_path_carrier_build()
}

pub async fn open_socks5_udp_association_target<
    OpenControl,
    OpenControlFut,
    ResolveRelay,
    ResolveRelayFut,
    RecordControl,
    OnClose,
>(
    target: Socks5ManagedUdpAssociationTarget,
    open_control: OpenControl,
    resolve_relay: ResolveRelay,
    record_control: RecordControl,
    on_close: OnClose,
) -> Result<Socks5UpstreamUdpAssociation, EngineError>
where
    OpenControl: FnOnce(&str, u16) -> OpenControlFut,
    OpenControlFut: Future<Output = Result<TokioSocket, EngineError>>,
    ResolveRelay: FnOnce(Address, u16) -> ResolveRelayFut,
    ResolveRelayFut: Future<Output = Result<(SocketAddress, TokioDatagramSocket), EngineError>>,
    RecordControl: FnOnce(&mut MeteredStream<TokioSocket>),
    OnClose: Fn(Socks5UpstreamAssociationCloseReason) + Send + Sync + 'static,
{
    Socks5UpstreamUdpAssociation::establish(
        target.into_protocol_target(),
        open_control,
        resolve_relay,
        record_control,
        on_close,
    )
    .await
}

pub async fn open_socks5_udp_packet_path_build<
    OpenControl,
    OpenControlFut,
    ResolveRelay,
    ResolveRelayFut,
    RecordControl,
    OnClose,
>(
    build: Socks5ManagedUdpPacketPathCarrierBuild,
    open_control: OpenControl,
    resolve_relay: ResolveRelay,
    record_control: RecordControl,
    on_close: OnClose,
) -> Result<Socks5UpstreamUdpAssociation, EngineError>
where
    OpenControl: FnOnce(&str, u16) -> OpenControlFut,
    OpenControlFut: Future<Output = Result<TokioSocket, EngineError>>,
    ResolveRelay: FnOnce(Address, u16) -> ResolveRelayFut,
    ResolveRelayFut: Future<Output = Result<(SocketAddress, TokioDatagramSocket), EngineError>>,
    RecordControl: FnOnce(&mut MeteredStream<TokioSocket>),
    OnClose: Fn(Socks5UpstreamAssociationCloseReason) + Send + Sync + 'static,
{
    open_socks5_udp_association_target(
        Socks5ManagedUdpAssociationTarget::new(
            socks5::udp::packet_path_carrier_association_target(build.into_protocol_build()),
        ),
        open_control,
        resolve_relay,
        record_control,
        on_close,
    )
    .await
}

pub async fn establish_registered_udp_association<R>(
    runtime: R,
    target: Socks5ManagedUdpAssociationTarget,
    session_id: u64,
) -> Result<Socks5UpstreamUdpAssociation, EngineError>
where
    R: Socks5UdpAssociationRuntime,
{
    let open_runtime = runtime.clone();
    let resolve_runtime = runtime.clone();
    let record_runtime = runtime.clone();

    open_socks5_udp_association_target(
        target,
        move |server, port| {
            let runtime = open_runtime.clone();
            let server = server.to_owned();
            async move { runtime.open_control_socket(&server, port).await }
        },
        move |relay_address, relay_port| {
            let runtime = resolve_runtime.clone();
            async move { runtime.resolve_udp_relay(relay_address, relay_port).await }
        },
        move |control| {
            record_runtime.record_control_traffic(session_id, control);
        },
        move |reason| {
            runtime.record_close(reason);
        },
    )
    .await
}

pub async fn establish_packet_path_udp_association<R>(
    runtime: R,
    build: Socks5ManagedUdpPacketPathCarrierBuild,
    session_id: u64,
) -> Result<Socks5UpstreamUdpAssociation, EngineError>
where
    R: Socks5UdpAssociationRuntime,
{
    let open_runtime = runtime.clone();
    let resolve_runtime = runtime.clone();
    let record_runtime = runtime.clone();

    open_socks5_udp_packet_path_build(
        build,
        move |server, port| {
            let runtime = open_runtime.clone();
            let server = server.to_owned();
            async move { runtime.open_control_socket(&server, port).await }
        },
        move |relay_address, relay_port| {
            let runtime = resolve_runtime.clone();
            async move { runtime.resolve_udp_relay(relay_address, relay_port).await }
        },
        move |control| {
            record_runtime.record_control_traffic(session_id, control);
        },
        move |reason| {
            runtime.record_close(reason);
        },
    )
    .await
}

pub struct Socks5UpstreamUdpAssociation {
    close_recorded: AtomicBool,
    on_close: Box<dyn Fn(Socks5UpstreamAssociationCloseReason) + Send + Sync>,
    association: socks5::udp::Socks5EstablishedUdpAssociation<
        MeteredStream<TokioSocket>,
        TokioDatagramSocket,
    >,
}

#[async_trait::async_trait]
pub trait Socks5UdpAssociationRuntime: Clone + Send + Sync + 'static {
    async fn open_control_socket(
        &self,
        server: &str,
        port: u16,
    ) -> Result<TokioSocket, EngineError>;

    async fn resolve_udp_relay(
        &self,
        relay_address: Address,
        relay_port: u16,
    ) -> Result<(SocketAddress, TokioDatagramSocket), EngineError>;

    fn record_control_traffic(&self, session_id: u64, control: &mut MeteredStream<TokioSocket>);

    fn record_close(&self, reason: Socks5UpstreamAssociationCloseReason);
}

impl Socks5UpstreamUdpAssociation {
    pub async fn establish<
        OpenControl,
        OpenControlFut,
        ResolveRelay,
        ResolveRelayFut,
        RecordControl,
        OnClose,
    >(
        target: socks5::udp::Socks5UdpAssociationTarget,
        open_control: OpenControl,
        resolve_relay: ResolveRelay,
        record_control: RecordControl,
        on_close: OnClose,
    ) -> Result<Self, EngineError>
    where
        OpenControl: FnOnce(&str, u16) -> OpenControlFut,
        OpenControlFut: Future<Output = Result<TokioSocket, EngineError>>,
        ResolveRelay: FnOnce(Address, u16) -> ResolveRelayFut,
        ResolveRelayFut: Future<Output = Result<(SocketAddress, TokioDatagramSocket), EngineError>>,
        RecordControl: FnOnce(&mut MeteredStream<TokioSocket>),
        OnClose: Fn(Socks5UpstreamAssociationCloseReason) + Send + Sync + 'static,
    {
        let association = target
            .establish_with_transport(
                |server, port| {
                    let server = server.to_owned();
                    async move {
                        let control = open_control(&server, port).await?;
                        Ok::<_, EngineError>(MeteredStream::new(control))
                    }
                },
                resolve_relay,
                record_control,
            )
            .await?;

        Ok(Self {
            close_recorded: AtomicBool::new(false),
            on_close: Box::new(on_close),
            association,
        })
    }

    pub fn close(self, reason: Socks5UpstreamAssociationCloseReason) {
        self.close_recorded.store(true, Ordering::Relaxed);
        (self.on_close)(reason);
    }

    pub async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.association
            .send_packet(target, port, payload)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }

    pub async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), EngineError> {
        self.association
            .recv_response_parts(buf)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }

    pub async fn recv_payload(&self, buf: &mut [u8]) -> Result<usize, EngineError> {
        self.association
            .recv_payload(buf)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }
}

impl Drop for Socks5UpstreamUdpAssociation {
    fn drop(&mut self) {
        if !self.close_recorded.load(Ordering::Relaxed) {
            self.close_recorded.store(true, Ordering::Relaxed);
            (self.on_close)(Socks5UpstreamAssociationCloseReason::Closed);
        }
    }
}
