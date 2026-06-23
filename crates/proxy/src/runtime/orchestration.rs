//! Shared runtime orchestration facts.
//!
//! This module does not implement protocol behavior. It only describes the
//! neutral routing facts that both TCP and UDP runtimes need after the engine
//! resolves an outbound target.

use zero_core::Address;

/// Resolved network endpoint for an outbound peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OutboundEndpoint<'a> {
    pub(crate) server: &'a str,
    pub(crate) port: u16,
}

impl OutboundEndpoint<'_> {
    pub(crate) fn upstream(self) -> (String, u16) {
        (self.server.to_owned(), self.port)
    }

    pub(crate) fn address(self) -> Address {
        Address::Domain(self.server.to_owned())
    }
}

/// UDP runtime transport category.
///
/// The dispatcher uses this to select a path family before handling the
/// concrete protocol variant. Adding protocols should normally extend a family
/// rather than create a protocol-pair-specific path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UdpPathCategory {
    Direct,
    Relay,
    StreamPacket,
    Datagram,
}

/// TCP runtime transport category.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TcpPathCategory {
    Direct,
    Block,
    Tunnel,
    Session,
    TransportSession,
}
