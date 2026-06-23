use std::net::SocketAddr;

use zero_engine::SessionOutcome;

use crate::protocol_runtime::udp::UdpPacketPathCarrier;
use crate::runtime::orchestration::UdpPathCategory;

/// Outbound type tracked per UDP flow.
///
/// Variant layout follows the path category model:
///
/// - **Direct path**: raw socket send, no upstream manager.
/// - **Relay path**: `Socks5` UDP ASSOCIATE relay through a control stream.
/// - **Stream packet path**: `Trojan`, `Mieru`: UDP packets sent over an
///   already established encrypted stream.
/// - **Datagram path**: `Shadowsocks`, `Hysteria2`: protocol datagrams
///   encoded and sent over a raw UDP socket or QUIC connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UdpFlowOutbound {
    // Direct path.
    Direct {
        tag: String,
        target_addr: SocketAddr,
    },

    // Relay path.
    Socks5 {
        tag: String,
        server: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },

    // Datagram path.
    #[cfg(feature = "shadowsocks")]
    Shadowsocks {
        tag: String,
        server: String,
        port: u16,
        password: String,
        cipher: String,
        packet_path_carrier: Option<UdpPacketPathCarrier>,
    },
    #[cfg(feature = "hysteria2")]
    Hysteria2 {
        tag: String,
        server: String,
        port: u16,
        password: String,
        client_fingerprint: Option<String>,
    },

    // Stream packet path.
    #[cfg(feature = "trojan")]
    Trojan {
        tag: String,
        server: String,
        port: u16,
        password: String,
        sni: Option<String>,
        insecure: bool,
        client_fingerprint: Option<String>,
        relay_chain: bool,
    },
    #[cfg(feature = "mieru")]
    Mieru {
        tag: String,
        server: String,
        port: u16,
        username: String,
        password: String,
        relay_chain: bool,
    },
}

pub(crate) struct Socks5UdpRelay<'a> {
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) username: Option<&'a str>,
    pub(crate) password: Option<&'a str>,
}

impl UdpFlowOutbound {
    pub(crate) fn tag(&self) -> &str {
        match self {
            Self::Direct { tag, .. } | Self::Socks5 { tag, .. } => tag,
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks { tag, .. } => tag,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2 { tag, .. } => tag,
            #[cfg(feature = "trojan")]
            Self::Trojan { tag, .. } => tag,
            #[cfg(feature = "mieru")]
            Self::Mieru { tag, .. } => tag,
        }
    }

    /// Return the path category for this outbound.
    pub(crate) fn path_category(&self) -> UdpPathCategory {
        match self {
            Self::Direct { .. } => UdpPathCategory::Direct,
            Self::Socks5 { .. } => UdpPathCategory::Relay,
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks { .. } => UdpPathCategory::Datagram,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2 { .. } => UdpPathCategory::Datagram,
            #[cfg(feature = "trojan")]
            Self::Trojan { .. } => UdpPathCategory::StreamPacket,
            #[cfg(feature = "mieru")]
            Self::Mieru { .. } => UdpPathCategory::StreamPacket,
        }
    }

    pub(super) fn direct_sender(&self) -> Option<SocketAddr> {
        self.direct_target_addr()
    }

    pub(crate) fn direct_target_addr(&self) -> Option<SocketAddr> {
        match self {
            Self::Direct { target_addr, .. } => Some(*target_addr),
            Self::Socks5 { .. } => None,
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks { .. } => None,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2 { .. } => None,
            #[cfg(feature = "trojan")]
            Self::Trojan { .. } => None,
            #[cfg(feature = "mieru")]
            Self::Mieru { .. } => None,
        }
    }

    pub(crate) fn socks5_relay(&self) -> Option<Socks5UdpRelay<'_>> {
        match self {
            Self::Socks5 {
                tag,
                server,
                port,
                username,
                password,
            } => Some(Socks5UdpRelay {
                tag,
                server,
                port: *port,
                username: username.as_deref(),
                password: password.as_deref(),
            }),
            Self::Direct { .. } => None,
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks { .. } => None,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2 { .. } => None,
            #[cfg(feature = "trojan")]
            Self::Trojan { .. } => None,
            #[cfg(feature = "mieru")]
            Self::Mieru { .. } => None,
        }
    }

    pub(super) fn upstream_response_tag(&self) -> Option<&str> {
        match self {
            Self::Direct { .. } => None,
            Self::Socks5 { tag, .. } => Some(tag),
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks { tag, .. } => Some(tag),
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2 { tag, .. } => Some(tag),
            #[cfg(feature = "trojan")]
            Self::Trojan { tag, .. } => Some(tag),
            #[cfg(feature = "mieru")]
            Self::Mieru { tag, .. } => Some(tag),
        }
    }

    pub(super) fn matches_upstream_tag(&self, outbound_tag: &str) -> bool {
        let Some(tag) = self.upstream_response_tag() else {
            return false;
        };
        tag == outbound_tag
    }

    pub(super) fn upstream_endpoint(&self) -> Option<(String, u16)> {
        match self {
            Self::Direct { .. } => None,
            Self::Socks5 { server, port, .. } => Some((server.clone(), *port)),
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks { server, port, .. } => Some((server.clone(), *port)),
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2 { server, port, .. } => Some((server.clone(), *port)),
            #[cfg(feature = "trojan")]
            Self::Trojan { server, port, .. } => Some((server.clone(), *port)),
            #[cfg(feature = "mieru")]
            Self::Mieru { server, port, .. } => Some((server.clone(), *port)),
        }
    }

    pub(super) fn success_outcome(&self) -> SessionOutcome {
        match self {
            Self::Direct { .. } => SessionOutcome::DirectRelayed,
            Self::Socks5 { .. } => SessionOutcome::ChainedRelayed,
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks { .. } => SessionOutcome::ChainedRelayed,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2 { .. } => SessionOutcome::ChainedRelayed,
            #[cfg(feature = "trojan")]
            Self::Trojan { .. } => SessionOutcome::ChainedRelayed,
            #[cfg(feature = "mieru")]
            Self::Mieru { .. } => SessionOutcome::ChainedRelayed,
        }
    }
}
