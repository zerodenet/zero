use core::future::Future;
use std::net::SocketAddr;

use crate::RuntimeError;
use zero_core::{Address, Session, UdpFlowPacket};
use zero_platform_tokio::TokioSocket;

use crate::TcpRelayStream;

#[derive(Debug, Clone)]
pub struct ManagedDatagramStartPlan<T> {
    pub tag: String,
    pub server: String,
    pub port: u16,
    pub resume: T,
}

impl<T> ManagedDatagramStartPlan<T> {
    pub fn new(tag: impl Into<String>, server: impl Into<String>, port: u16, resume: T) -> Self {
        Self {
            tag: tag.into(),
            server: server.into(),
            port,
            resume,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManagedStreamPacketBridgePlan<T> {
    pub tag: String,
    pub server: String,
    pub port: u16,
    pub resume: T,
    pub relay_chain: bool,
}

impl<T> ManagedStreamPacketBridgePlan<T> {
    pub fn new(
        tag: impl Into<String>,
        server: impl Into<String>,
        port: u16,
        resume: T,
        relay_chain: bool,
    ) -> Self {
        Self {
            tag: tag.into(),
            server: server.into(),
            port,
            resume,
            relay_chain,
        }
    }
}

pub trait ProtocolManagedStreamConnectorParts {
    fn into_managed_connector_parts(self) -> (String, bool);
}

pub trait ManagedConnectorFlowOps {
    fn into_managed_connector_parts(self) -> (String, bool);
}

pub struct ManagedConnectorFlow<T>(pub T);

impl<T> ProtocolManagedStreamConnectorParts for ManagedConnectorFlow<T>
where
    T: ManagedConnectorFlowOps,
{
    fn into_managed_connector_parts(self) -> (String, bool) {
        self.0.into_managed_connector_parts()
    }
}

pub trait ProtocolManagedStreamUdpResumeMetadata {
    const ESTABLISH_STAGE: &'static str;
    const RELAY_UPSTREAM_STAGE: &'static str;
    const RELAY_ESTABLISH_STAGE: &'static str;
    const RELAY_SEND_STAGE: &'static str;
    const MISMATCH_STAGE: &'static str;
    const MISMATCH_MESSAGE: &'static str;
}

pub trait ProtocolManagedTupleUdpResumeMetadata {
    const ESTABLISH_STAGE: &'static str;
    const RELAY_UPSTREAM_STAGE: &'static str;
    const RELAY_ESTABLISH_STAGE: &'static str;
    const RELAY_SEND_STAGE: &'static str;
    const MISMATCH_STAGE: &'static str;
    const MISMATCH_MESSAGE: &'static str;
}

pub trait ProtocolManagedPacketUdpResumeMetadata {
    const ESTABLISH_STAGE: &'static str;
    const RELAY_UPSTREAM_STAGE: &'static str;
    const RELAY_ESTABLISH_STAGE: &'static str;
    const RELAY_SEND_STAGE: &'static str;
    const MISMATCH_STAGE: &'static str;
    const MISMATCH_MESSAGE: &'static str;
}

#[derive(Debug, Clone, Copy)]
pub struct ProtocolManagedStreamFlowStages {
    pub establish_stage: &'static str,
    pub relay_upstream_stage: &'static str,
    pub relay_establish_stage: &'static str,
    pub relay_send_stage: &'static str,
    pub mismatch_stage: &'static str,
    pub mismatch_message: &'static str,
}

impl ProtocolManagedStreamFlowStages {
    pub fn from_resume<T>() -> Self
    where
        T: ProtocolManagedStreamUdpResumeMetadata,
    {
        Self {
            establish_stage: T::ESTABLISH_STAGE,
            relay_upstream_stage: T::RELAY_UPSTREAM_STAGE,
            relay_establish_stage: T::RELAY_ESTABLISH_STAGE,
            relay_send_stage: T::RELAY_SEND_STAGE,
            mismatch_stage: T::MISMATCH_STAGE,
            mismatch_message: T::MISMATCH_MESSAGE,
        }
    }
}

pub trait ProtocolManagedDatagramUdpResumeMetadata {
    const ESTABLISH_STAGE: &'static str;
    const MISMATCH_STAGE: &'static str;
    const MISMATCH_MESSAGE: &'static str;
}

#[async_trait::async_trait]
pub trait ProtocolManagedDatagramUdpResumeConnectionOps:
    ProtocolManagedDatagramUdpResumeMetadata + Send + Sync + std::fmt::Debug + Clone + 'static
{
    type RawConnection: ManagedTupleUdpConnectionOps;

    fn connector_flow_cache_key(&self, server: &str, port: u16) -> String;

    async fn open_protocol_connection(
        &self,
        server: &str,
        port: u16,
        target: &Address,
        target_port: u16,
        payload: &[u8],
    ) -> Result<Self::RawConnection, RuntimeError>;
}

#[async_trait::async_trait]
pub trait ProtocolManagedDatagramSocketUdpResumeConnectionOps:
    ProtocolManagedDatagramUdpResumeMetadata + Send + Sync + std::fmt::Debug + Clone + 'static
{
    type RawConnection: ManagedDatagramConnectionOps;

    const SEND_STAGE: &'static str;
    const RESOLVE_UPSTREAM_MESSAGE: &'static str;
    const PROXY_CONTEXT_MESSAGE: &'static str = "expected proxy context for managed datagram flow";

    fn connector_flow_cache_key(&self, server: &str, port: u16) -> String;

    async fn open_protocol_connection(
        &self,
        endpoint: SocketAddr,
    ) -> Result<Self::RawConnection, RuntimeError>;
}

#[derive(Debug, Clone)]
pub struct ManagedTupleUdpResume<T>(pub T);

impl<T> ManagedTupleUdpResume<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> ProtocolManagedStreamUdpResumeMetadata for ManagedTupleUdpResume<T>
where
    T: ProtocolManagedTupleUdpResumeMetadata,
{
    const ESTABLISH_STAGE: &'static str = T::ESTABLISH_STAGE;
    const RELAY_UPSTREAM_STAGE: &'static str = T::RELAY_UPSTREAM_STAGE;
    const RELAY_ESTABLISH_STAGE: &'static str = T::RELAY_ESTABLISH_STAGE;
    const RELAY_SEND_STAGE: &'static str = T::RELAY_SEND_STAGE;
    const MISMATCH_STAGE: &'static str = T::MISMATCH_STAGE;
    const MISMATCH_MESSAGE: &'static str = T::MISMATCH_MESSAGE;
}

#[derive(Debug, Clone)]
pub struct ManagedPacketUdpResume<T>(pub T);

impl<T> ManagedPacketUdpResume<T> {
    pub fn new(inner: T) -> Self {
        Self(inner)
    }
}

impl<T> ProtocolManagedStreamUdpResumeMetadata for ManagedPacketUdpResume<T>
where
    T: ProtocolManagedPacketUdpResumeMetadata,
{
    const ESTABLISH_STAGE: &'static str = T::ESTABLISH_STAGE;
    const RELAY_UPSTREAM_STAGE: &'static str = T::RELAY_UPSTREAM_STAGE;
    const RELAY_ESTABLISH_STAGE: &'static str = T::RELAY_ESTABLISH_STAGE;
    const RELAY_SEND_STAGE: &'static str = T::RELAY_SEND_STAGE;
    const MISMATCH_STAGE: &'static str = T::MISMATCH_STAGE;
    const MISMATCH_MESSAGE: &'static str = T::MISMATCH_MESSAGE;
}

pub trait ProtocolManagedStreamUdpLeafOps {
    type Resume: Send + Sync + std::fmt::Debug + 'static;

    fn direct_udp_resume(&self) -> Self::Resume;

    fn relay_final_hop_udp_resume(&self) -> Self::Resume;
}

pub trait ProtocolRelayTwoStreamManagedUdpLeafOps: ProtocolManagedStreamUdpLeafOps {
    fn udp_relay_needs_two_streams(&self) -> bool;

    fn relay_two_stream_udp_resume(&self) -> Self::Resume;
}

#[async_trait::async_trait]
pub trait ProtocolManagedTupleUdpFlowResumeConnectionOps:
    Send + Sync + std::fmt::Debug + 'static
{
    type ConnectorFlow: ProtocolManagedStreamConnectorParts;
    type RawConnection: ManagedTupleUdpConnectionOps;

    fn connector_flow_for_resume(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Self::ConnectorFlow;

    async fn open_direct_protocol_connection<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<Self::RawConnection, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send;

    async fn open_relay_protocol_connection(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        tls_server_name: Option<&str>,
    ) -> Result<Self::RawConnection, RuntimeError>;
}

#[async_trait::async_trait]
pub trait ProtocolManagedPacketUdpFlowResumeConnectionOps:
    Send + Sync + std::fmt::Debug + 'static
{
    type ConnectorFlow: ProtocolManagedStreamConnectorParts;
    type RawConnection: ManagedPacketUdpConnectionOps;

    fn connector_flow_for_resume(
        &self,
        server: &str,
        port: u16,
        session_id: u64,
    ) -> Self::ConnectorFlow;

    async fn open_direct_protocol_connection<OpenSocket, OpenSocketFut>(
        &self,
        session: &Session,
        open_socket: OpenSocket,
    ) -> Result<Self::RawConnection, RuntimeError>
    where
        OpenSocket: Clone + Fn(&str, u16) -> OpenSocketFut + Send + Sync,
        OpenSocketFut: Future<Output = Result<TokioSocket, RuntimeError>> + Send;

    async fn open_relay_protocol_connection(
        &self,
        stream: TcpRelayStream,
        session: &Session,
        tls_server_name: Option<&str>,
    ) -> Result<Self::RawConnection, RuntimeError>;
}

#[async_trait::async_trait]
pub trait ManagedTupleUdpConnectionOps: Send + Sync + 'static {
    type SendError: std::fmt::Display + Send + Sync + 'static;

    async fn send_protocol_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Self::SendError>;

    fn subscribe_protocol_packets(
        &self,
    ) -> tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)>;

    fn closed_message_for_connection(&self) -> &'static str;
}

#[async_trait::async_trait]
pub trait ManagedPacketUdpConnectionOps: Send + Sync + 'static {
    type SendError: std::fmt::Display + Send + Sync + 'static;

    async fn send_protocol_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Self::SendError>;

    fn subscribe_protocol_packets(&self) -> tokio::sync::broadcast::Receiver<UdpFlowPacket>;

    fn closed_message_for_connection(&self) -> &'static str;
}

#[async_trait::async_trait]
pub trait ManagedDatagramConnectionOps: Send + Sync + 'static {
    type SendError: std::fmt::Display + Send + Sync + 'static;

    async fn send_protocol_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), Self::SendError>;

    fn subscribe_protocol_datagrams(
        &self,
    ) -> tokio::sync::broadcast::Receiver<(Address, u16, Vec<u8>)>;

    fn closed_message_for_datagram_connection(&self) -> &'static str;
}
