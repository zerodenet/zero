//! Shared runtime orchestration facts.
//!
//! This module does not implement protocol behavior. It only describes the
//! neutral routing facts that both TCP and UDP runtimes need after the engine
//! resolves an outbound target.

use zero_core::Address;
use zero_engine::ResolvedLeafOutbound;

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

/// Return the tag used for runtime health tracking.
pub(crate) fn health_tag<'a>(candidate: &ResolvedLeafOutbound<'a>) -> Option<&'a str> {
    match candidate {
        ResolvedLeafOutbound::Direct { .. } | ResolvedLeafOutbound::Block { .. } => None,
        ResolvedLeafOutbound::Socks5 { tag, .. }
        | ResolvedLeafOutbound::Vless { tag, .. }
        | ResolvedLeafOutbound::Hysteria2 { tag, .. }
        | ResolvedLeafOutbound::Shadowsocks { tag, .. }
        | ResolvedLeafOutbound::Trojan { tag, .. }
        | ResolvedLeafOutbound::Vmess { tag, .. }
        | ResolvedLeafOutbound::Mieru { tag, .. } => Some(tag),
    }
}

/// Return the remote endpoint for outbounds that dial a peer.
pub(crate) fn endpoint<'a>(candidate: &ResolvedLeafOutbound<'a>) -> Option<OutboundEndpoint<'a>> {
    match candidate {
        ResolvedLeafOutbound::Socks5 { server, port, .. }
        | ResolvedLeafOutbound::Vless { server, port, .. }
        | ResolvedLeafOutbound::Hysteria2 { server, port, .. }
        | ResolvedLeafOutbound::Shadowsocks { server, port, .. }
        | ResolvedLeafOutbound::Trojan { server, port, .. }
        | ResolvedLeafOutbound::Vmess { server, port, .. }
        | ResolvedLeafOutbound::Mieru { server, port, .. } => Some(OutboundEndpoint {
            server,
            port: *port,
        }),
        ResolvedLeafOutbound::Direct { .. } | ResolvedLeafOutbound::Block { .. } => None,
    }
}

/// Return the TCP orchestration category for one resolved outbound.
pub(crate) fn tcp_path_category(candidate: &ResolvedLeafOutbound<'_>) -> TcpPathCategory {
    match candidate {
        ResolvedLeafOutbound::Direct { .. } => TcpPathCategory::Direct,
        ResolvedLeafOutbound::Block { .. } => TcpPathCategory::Block,
        ResolvedLeafOutbound::Socks5 { .. }
        | ResolvedLeafOutbound::Vless { .. }
        | ResolvedLeafOutbound::Trojan { .. }
        | ResolvedLeafOutbound::Vmess { .. } => TcpPathCategory::Tunnel,
        ResolvedLeafOutbound::Shadowsocks { .. } | ResolvedLeafOutbound::Mieru { .. } => {
            TcpPathCategory::Session
        }
        ResolvedLeafOutbound::Hysteria2 { .. } => TcpPathCategory::TransportSession,
    }
}
