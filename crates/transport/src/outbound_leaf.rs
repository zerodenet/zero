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

pub trait ProtocolTcpTransportLeafMetadata: ProtocolTransportLeaf {
    const TCP_CONNECT_STAGE: &'static str;
    const TCP_INVALID_CONNECT_CONFIG: &'static str;
    const TCP_INVALID_RELAY_CONFIG: &'static str;
}

#[async_trait::async_trait]
#[async_trait::async_trait]
pub trait ProtocolTcpTransportLeafOps: ProtocolTcpTransportLeafMetadata + Send + Sync {
    type Opened: ProtocolTcpTransportOpenResult;

    async fn open_tcp_stream<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<Self::Opened, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send;

    async fn open_tcp_relay_hop(
        &self,
        stream: TcpRelayStream,
        session: &Session,
    ) -> Result<TcpRelayStream, RuntimeError>;
}
pub trait ProtocolUdpTransportLeafMetadata: ProtocolTransportLeaf {
    const UDP_DIRECT_STAGE: &'static str;
    const UDP_INVALID_CONFIG: &'static str;
    const UDP_RELAY_FINAL_STAGE: &'static str;
}

pub trait ProtocolRelayTwoStreamUdpTransportLeafMetadata: ProtocolUdpTransportLeafMetadata {
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

pub struct PreparedTransportLeaf<TLeaf> {
    leaf: TLeaf,
}

impl<TLeaf> PreparedTransportLeaf<TLeaf> {
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

impl<TLeaf> PreparedTransportLeaf<TLeaf>
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

pub async fn open_prepared_tcp_transport_leaf_stream<TLeaf, OpenSocket, OpenSocketFut>(
    session: &Session,
    prepared: &PreparedTransportLeaf<TLeaf>,
    open_socket: OpenSocket,
) -> Result<TLeaf::Opened, RuntimeError>
where
    TLeaf: ProtocolTcpTransportLeafOps,
    OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
    OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
{
    open_tcp_transport_leaf_stream(session, prepared.leaf(), open_socket).await
}

pub async fn open_prepared_tcp_transport_leaf_relay_hop<TLeaf>(
    stream: TcpRelayStream,
    session: &Session,
    prepared: &PreparedTransportLeaf<TLeaf>,
) -> Result<TcpRelayStream, RuntimeError>
where
    TLeaf: ProtocolTcpTransportLeafOps,
{
    open_tcp_transport_leaf_relay_hop(stream, session, prepared.leaf()).await
}

pub fn prepared_direct_udp_resume<TLeaf>(prepared: &PreparedTransportLeaf<TLeaf>) -> TLeaf::Resume
where
    TLeaf: crate::managed_udp::ProtocolManagedStreamUdpLeafOps,
{
    prepared.leaf().direct_udp_resume()
}

pub fn prepared_relay_final_hop_udp_resume<TLeaf>(
    prepared: &PreparedTransportLeaf<TLeaf>,
) -> TLeaf::Resume
where
    TLeaf: crate::managed_udp::ProtocolManagedStreamUdpLeafOps,
{
    prepared.leaf().relay_final_hop_udp_resume()
}

pub fn prepared_udp_relay_needs_two_streams<TLeaf>(prepared: &PreparedTransportLeaf<TLeaf>) -> bool
where
    TLeaf: crate::managed_udp::ProtocolRelayTwoStreamManagedUdpLeafOps,
{
    prepared.leaf().udp_relay_needs_two_streams()
}

pub fn prepared_relay_two_stream_udp_resume<TLeaf>(
    prepared: &PreparedTransportLeaf<TLeaf>,
) -> TLeaf::Resume
where
    TLeaf: crate::managed_udp::ProtocolRelayTwoStreamManagedUdpLeafOps,
{
    prepared.leaf().relay_two_stream_udp_resume()
}

pub async fn open_prepared_relay_two_stream_udp_transport<TLeaf>(
    prepared: &PreparedTransportLeaf<TLeaf>,
    post_stream: TcpRelayStream,
    get_stream: TcpRelayStream,
) -> Result<TcpRelayStream, RuntimeError>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf,
{
    open_relay_two_stream_udp_transport(prepared.leaf(), post_stream, get_stream).await
}

pub async fn open_tcp_transport_leaf_stream<TLeaf, OpenSocket, OpenSocketFut>(
    session: &Session,
    leaf: &TLeaf,
    open_socket: OpenSocket,
) -> Result<TLeaf::Opened, RuntimeError>
where
    TLeaf: ProtocolTcpTransportLeafOps,
    OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
    OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send,
{
    leaf.open_tcp_stream(session, open_socket).await
}

pub async fn open_tcp_transport_leaf_relay_hop<TLeaf>(
    stream: TcpRelayStream,
    session: &Session,
    leaf: &TLeaf,
) -> Result<TcpRelayStream, RuntimeError>
where
    TLeaf: ProtocolTcpTransportLeafOps,
{
    leaf.open_tcp_relay_hop(stream, session).await
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
