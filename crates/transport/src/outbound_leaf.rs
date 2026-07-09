use std::future::Future;
use std::path::Path;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
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

pub enum ResolveTransportLeafError<E> {
    InvalidConfig(E),
    MissingLeaf,
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

    fn validate_udp_relay_final_hop(&self) -> Result<(), EngineError> {
        Ok(())
    }
}

pub trait ProtocolTransportLeafResolver<'a> {
    type TransportLeaf: ProtocolTransportLeaf + 'a;
    type ResolveError: std::fmt::Display;

    fn resolve_transport_leaf(
        &self,
        source_dir: Option<&Path>,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Result<Option<Self::TransportLeaf>, Self::ResolveError>;
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
    ) -> Result<Self::Opened, EngineError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send;

    async fn open_tcp_relay_hop_for_leaf(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        leaf: &TLeaf,
    ) -> Result<TcpRelayStream, EngineError>;
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
    ) -> Result<TcpRelayStream, EngineError>;

    fn needs_relay_two_streams(&self) -> bool;
}

pub fn resolve_transport_leaf<'a, T, E, F>(
    source_dir: Option<&Path>,
    leaf: &ResolvedLeafOutbound<'a>,
    build: F,
) -> Result<T, ResolveTransportLeafError<E>>
where
    F: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<T>, E>,
{
    build(source_dir, leaf)
        .map_err(ResolveTransportLeafError::InvalidConfig)?
        .ok_or(ResolveTransportLeafError::MissingLeaf)
}

pub fn resolve_last_transport_leaf<'a, 'chain, T, E, F>(
    chain: &'chain [ResolvedLeafOutbound<'a>],
    source_dir: Option<&Path>,
    build: F,
) -> Result<T, ResolveTransportLeafError<E>>
where
    F: FnOnce(Option<&Path>, &ResolvedLeafOutbound<'a>) -> Result<Option<T>, E>,
{
    let leaf = chain.last().ok_or(ResolveTransportLeafError::MissingLeaf)?;
    resolve_transport_leaf(source_dir, leaf, build)
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

pub async fn open_tcp_transport_bridge_stream<TLeaf, TBridge, OpenSocket, OpenSocketFut>(
    bridge: &TBridge,
    session: &Session,
    leaf: &TLeaf,
    open_socket: OpenSocket,
) -> Result<TBridge::Opened, EngineError>
where
    TBridge: ProtocolTcpTransportBridgeOps<TLeaf>,
    OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
    OpenSocketFut: Future<Output = Result<TokioSocket, EngineError>> + Send,
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
) -> Result<TcpRelayStream, EngineError>
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
) -> Result<TcpRelayStream, EngineError>
where
    TLeaf: ProtocolRelayTwoStreamTransportLeaf,
{
    leaf.open_relay_two_stream_udp_transport(post_stream, get_stream)
        .await
}
