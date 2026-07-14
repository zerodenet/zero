use std::future::Future;

use crate::RuntimeError;
use zero_core::Session;
use zero_platform_tokio::TokioSocket;

use crate::{StreamTraffic, TcpRelayStream};

pub fn clone_socket_opener<OpenSocket, OpenSocketFut>(
    open_socket: OpenSocket,
) -> impl Clone + Fn(&str, u16) -> OpenSocketFut
where
    OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut,
{
    move |server, port| open_socket.clone()(server, port)
}

#[derive(Clone, Copy)]
pub struct TransportLeafEndpoint<'a> {
    pub tag: &'a str,
    pub server: &'a str,
    pub port: u16,
}

pub trait ProtocolTransportLeaf {
    fn tag(&self) -> &str;

    fn server(&self) -> &str;

    fn port(&self) -> u16;

    fn validate_udp_relay_final_hop(&self) -> Result<(), RuntimeError> {
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait ProtocolSocketTcpHandshake: ProtocolTransportLeaf + Send + Sync {
    fn connect_stage(&self) -> &'static str;

    async fn handshake_socket(
        &self,
        socket: TokioSocket,
        session: &Session,
    ) -> Result<(TcpRelayStream, StreamTraffic), RuntimeError>;

    async fn handshake_relay(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError>;
}

#[async_trait::async_trait]
pub trait ProtocolSessionTcpHandshake: ProtocolTransportLeaf + Send + Sync {
    fn connect_stage(&self) -> &'static str;

    async fn connect_session_stream(
        &self,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError>;
}

pub trait ProtocolTcpTransportOpenResult {
    fn into_proxied_stream_parts(self) -> (TcpRelayStream, StreamTraffic);
}

pub trait ProtocolTcpTransportBridgeMetadata {
    const TCP_CONNECT_STAGE: &'static str;
    const TCP_INVALID_CONNECT_CONFIG: &'static str;
    const TCP_INVALID_CONNECT_LEAF_STAGE: &'static str;
    const TCP_INVALID_RELAY_CONFIG: &'static str;
    const TCP_INVALID_RELAY_LEAF_STAGE: &'static str;
    const EXPECTED_OUTBOUND_LEAF: &'static str;
}

#[async_trait::async_trait]
pub trait ProtocolTcpTransportBridgeOps<TLeaf>: Send + Sync {
    type Opened: ProtocolTcpTransportOpenResult;

    async fn open_tcp_stream_for_leaf<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        leaf: &TLeaf,
        open_socket: OpenSocket,
    ) -> Result<Self::Opened, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send;

    async fn open_tcp_relay_hop_for_leaf(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        leaf: &TLeaf,
    ) -> Result<TcpRelayStream, RuntimeError>;
}
pub trait ProtocolUdpTransportBridgeMetadata {
    const UDP_DIRECT_STAGE: &'static str;
    const UDP_INVALID_CONFIG: &'static str;
    const UDP_RELAY_FINAL_STAGE: &'static str;
    const EXPECTED_OUTBOUND_LEAF: &'static str;
}

pub trait ProtocolRelayTwoStreamUdpTransportBridgeMetadata:
    ProtocolUdpTransportBridgeMetadata
{
    const UDP_RELAY_CAPABILITY_STAGE: &'static str;
    const UDP_RELAY_CHAIN_STAGE: &'static str;
}

#[async_trait::async_trait]
pub trait ProtocolRelayTwoStreamTransportLeaf: ProtocolTransportLeaf {
    async fn open_relay_two_stream_udp_transport(
        &self,
        post_stream: TcpRelayStream,
        get_stream: TcpRelayStream,
    ) -> Result<TcpRelayStream, RuntimeError>;

    fn needs_relay_two_streams(&self) -> bool;
}

pub fn transport_leaf_endpoint<TLeaf>(leaf: &TLeaf) -> TransportLeafEndpoint<'_>
where
    TLeaf: ProtocolTransportLeaf,
{
    TransportLeafEndpoint {
        tag: leaf.tag(),
        server: leaf.server(),
        port: leaf.port(),
    }
}

pub struct PreparedTransportBridgeLeaf<TLeaf> {
    leaf: TLeaf,
}

impl<TLeaf> PreparedTransportBridgeLeaf<TLeaf> {
    pub fn new(leaf: TLeaf) -> Self {
        Self { leaf }
    }

    pub fn leaf(&self) -> &TLeaf {
        &self.leaf
    }

    pub fn into_leaf(self) -> TLeaf {
        self.leaf
    }
}

impl<TLeaf> PreparedTransportBridgeLeaf<TLeaf>
where
    TLeaf: ProtocolTransportLeaf,
{
    pub fn endpoint(&self) -> TransportLeafEndpoint<'_> {
        transport_leaf_endpoint(self.leaf())
    }

    pub fn validate_udp_relay_final_hop(&self) -> Result<(), RuntimeError> {
        self.leaf().validate_udp_relay_final_hop()
    }
}

pub async fn open_prepared_tcp_transport_bridge_stream<TLeaf, TBridge, OpenSocket, OpenSocketFut>(
    bridge: &TBridge,
    session: &Session,
    prepared: &PreparedTransportBridgeLeaf<TLeaf>,
    open_socket: OpenSocket,
) -> Result<TBridge::Opened, RuntimeError>
where
    TBridge: ProtocolTcpTransportBridgeOps<TLeaf>,
    OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
    OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
{
    open_tcp_transport_bridge_stream(bridge, session, prepared.leaf(), open_socket).await
}

pub async fn open_prepared_tcp_transport_bridge_relay_hop<TLeaf, TBridge>(
    bridge: &TBridge,
    stream: TcpRelayStream,
    session: &Session,
    prepared: &PreparedTransportBridgeLeaf<TLeaf>,
) -> Result<TcpRelayStream, RuntimeError>
where
    TBridge: ProtocolTcpTransportBridgeOps<TLeaf>,
{
    open_tcp_transport_bridge_relay_hop(bridge, stream, session, prepared.leaf()).await
}

pub fn prepared_direct_udp_resume<TLeaf, TBridge>(
    bridge: &TBridge,
    prepared: &PreparedTransportBridgeLeaf<TLeaf>,
) -> TBridge::Resume
where
    TBridge: crate::managed_udp::ProtocolManagedStreamUdpBridgeOps<TLeaf>,
{
    bridge.direct_udp_resume_for_leaf(prepared.leaf())
}

pub fn prepared_relay_final_hop_udp_resume<TLeaf, TBridge>(
    bridge: &TBridge,
    prepared: &PreparedTransportBridgeLeaf<TLeaf>,
) -> TBridge::Resume
where
    TBridge: crate::managed_udp::ProtocolManagedStreamUdpBridgeOps<TLeaf>,
{
    bridge.relay_final_hop_udp_resume_for_leaf(prepared.leaf())
}

pub fn prepared_udp_relay_needs_two_streams<TLeaf, TBridge>(
    bridge: &TBridge,
    prepared: &PreparedTransportBridgeLeaf<TLeaf>,
) -> bool
where
    TBridge: crate::managed_udp::ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
{
    bridge.udp_relay_needs_two_streams_for_leaf(prepared.leaf())
}

pub fn prepared_relay_two_stream_udp_resume<TLeaf, TBridge>(
    bridge: &TBridge,
    prepared: &PreparedTransportBridgeLeaf<TLeaf>,
) -> TBridge::Resume
where
    TBridge: crate::managed_udp::ProtocolRelayTwoStreamManagedUdpBridgeOps<TLeaf>,
{
    bridge.relay_two_stream_udp_resume_for_leaf(prepared.leaf())
}

pub async fn open_prepared_relay_two_stream_udp_transport<TLeaf>(
    prepared: &PreparedTransportBridgeLeaf<TLeaf>,
    post_stream: TcpRelayStream,
    get_stream: TcpRelayStream,
) -> Result<TcpRelayStream, RuntimeError>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf,
{
    open_relay_two_stream_udp_transport(prepared.leaf(), post_stream, get_stream).await
}

pub async fn open_tcp_transport_bridge_stream<TLeaf, TBridge, OpenSocket, OpenSocketFut>(
    bridge: &TBridge,
    session: &Session,
    leaf: &TLeaf,
    open_socket: OpenSocket,
) -> Result<TBridge::Opened, RuntimeError>
where
    TBridge: ProtocolTcpTransportBridgeOps<TLeaf>,
    OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
    OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
{
    bridge
        .open_tcp_stream_for_leaf(session, leaf, open_socket)
        .await
}

pub async fn open_tcp_transport_bridge_relay_hop<TLeaf, TBridge>(
    bridge: &TBridge,
    stream: TcpRelayStream,
    session: &Session,
    leaf: &TLeaf,
) -> Result<TcpRelayStream, RuntimeError>
where
    TBridge: ProtocolTcpTransportBridgeOps<TLeaf>,
{
    bridge
        .open_tcp_relay_hop_for_leaf(stream, session, leaf)
        .await
}

pub async fn open_relay_two_stream_udp_transport<TLeaf>(
    leaf: &TLeaf,
    post_stream: TcpRelayStream,
    get_stream: TcpRelayStream,
) -> Result<TcpRelayStream, RuntimeError>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf,
{
    leaf.open_relay_two_stream_udp_transport(post_stream, get_stream)
        .await
}
