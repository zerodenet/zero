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
