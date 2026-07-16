#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::path::UdpPathCategory;

use super::super::model::{ManagedUdpFlowRef, UdpFlowOutbound, UdpFlowUpstream};

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl UdpFlowOutbound {
    pub(crate) fn tag(&self) -> &str {
        match self {
            Self::Direct { tag, .. } => tag,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            Self::PacketPathDatagram { tag, .. } => tag,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { tag, .. } => tag,
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            Self::Datagram { tag, .. } => tag,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { tag, .. } => tag,
        }
    }

    pub(crate) fn path_category(&self) -> UdpPathCategory {
        match self {
            Self::Direct { .. } => UdpPathCategory::Direct,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { .. } => UdpPathCategory::Relay,
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            Self::Datagram { .. } => UdpPathCategory::Datagram,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { .. } => UdpPathCategory::StreamPacket,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            Self::PacketPathDatagram { .. } => UdpPathCategory::PacketPathDatagram,
        }
    }

    pub(crate) fn direct_target_addr(&self) -> Option<std::net::SocketAddr> {
        match self {
            Self::Direct { target_addr, .. } => Some(*target_addr),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            Self::PacketPathDatagram { .. } => None,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { .. } => None,
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            Self::Datagram { .. } => None,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { .. } => None,
        }
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(crate) fn relay_managed_flow(&self) -> Option<ManagedUdpFlowRef> {
        match self {
            Self::Relay { managed, .. } => Some(*managed),
            Self::Direct { .. } => None,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            Self::PacketPathDatagram { .. } => None,
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            Self::Datagram { .. } => None,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { .. } => None,
        }
    }

    #[cfg(any(
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) fn managed_flow(&self) -> Option<ManagedUdpFlowRef> {
        match self {
            Self::Direct { .. } => None,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            Self::PacketPathDatagram { .. } => None,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { managed, .. } => Some(*managed),
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            Self::Datagram { managed, .. } => Some(*managed),
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { managed, .. } => Some(*managed),
        }
    }

    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) fn packet_path_snapshot(
        &self,
    ) -> Option<&crate::runtime::udp_flow::packet_path::PacketPathFlowSnapshot> {
        match self {
            Self::PacketPathDatagram { snapshot, .. } => Some(snapshot),
            Self::Direct { .. } => None,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { .. } => None,
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            Self::Datagram { .. } => None,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { .. } => None,
        }
    }

    pub(crate) fn upstream(&self) -> Option<UdpFlowUpstream<'_>> {
        match self {
            Self::Direct { .. } => None,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            Self::PacketPathDatagram { server, port, .. } => Some(UdpFlowUpstream {
                server,
                port: *port,
            }),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { server, port, .. } => Some(UdpFlowUpstream {
                server,
                port: *port,
            }),
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            Self::Datagram { server, port, .. } => Some(UdpFlowUpstream {
                server,
                port: *port,
            }),
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { server, port, .. } => Some(UdpFlowUpstream {
                server,
                port: *port,
            }),
        }
    }
}
