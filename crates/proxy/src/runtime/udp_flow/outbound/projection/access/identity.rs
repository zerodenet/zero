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

use crate::runtime::udp_flow::outbound::model::UdpFlowOutbound;

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
}
