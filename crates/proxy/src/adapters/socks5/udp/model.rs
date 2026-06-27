use std::sync::Arc;

use zero_core::Address;
use zero_engine::EngineError;

/// SOCKS5 UDP association close reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UpstreamAssociationCloseReason {
    Closed,
    IdleTimeout,
    Dropped,
}

pub(crate) struct Socks5UdpAssociationView<'a> {
    pub(crate) outbound_tag: &'a str,
}

pub(crate) struct ClosedSocks5UdpAssociation {
    pub(crate) outbound_tag: String,
    pub(crate) server: String,
    pub(crate) port: u16,
}

/// SOCKS5 UDP association context.
#[derive(Clone)]
pub(super) struct Socks5UdpAssociation {
    pub(super) outbound_tag: String,
    pub(super) server: String,
    pub(super) port: u16,
    pub(super) config: socks5::Socks5OwnedUdpAssociationConfig,
}

#[async_trait::async_trait]
pub(super) trait Socks5UdpAssociationHandle: Send + Sync {
    fn matches(&self, outbound_tag: &str, server: &str, port: u16) -> bool;

    fn outbound_tag(&self) -> &str;

    fn upstream_endpoint(&self) -> (&str, u16);

    fn close(self: Box<Self>, reason: UpstreamAssociationCloseReason);

    async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError>;

    async fn recv_packet(&self, buf: &mut [u8]) -> Result<usize, EngineError>;
}

pub(super) type BoxedSocks5UdpAssociation = Box<dyn Socks5UdpAssociationHandle>;

#[async_trait::async_trait]
pub(super) trait Socks5UdpPacketPathAssociation: Send + Sync {
    async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError>;

    async fn recv_payload(&self, buf: &mut [u8]) -> Result<usize, EngineError>;
}

pub(super) type SharedSocks5UdpPacketPathAssociation = Arc<dyn Socks5UdpPacketPathAssociation>;
