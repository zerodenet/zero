#[cfg(feature = "udp-runtime")]
use crate::runtime::path::UdpPathCategory;

use crate::runtime::udp_flow::outbound::model::UdpFlowOutbound;

#[cfg(feature = "udp-runtime")]

impl UdpFlowOutbound {
    pub(crate) fn tag(&self) -> &str {
        match self {
            Self::Direct { tag, .. } => tag,
            #[cfg(feature = "udp-runtime")]
            Self::PacketPathDatagram { tag, .. } => tag,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { tag, .. } => tag,
            #[cfg(feature = "managed-datagram-runtime")]
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
            #[cfg(feature = "managed-datagram-runtime")]
            Self::Datagram { .. } => UdpPathCategory::Datagram,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { .. } => UdpPathCategory::StreamPacket,
            #[cfg(feature = "udp-runtime")]
            Self::PacketPathDatagram { .. } => UdpPathCategory::PacketPathDatagram,
        }
    }

    pub(crate) fn direct_target_addr(&self) -> Option<std::net::SocketAddr> {
        match self {
            Self::Direct { target_addr, .. } => Some(*target_addr),
            #[cfg(feature = "udp-runtime")]
            Self::PacketPathDatagram { .. } => None,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { .. } => None,
            #[cfg(feature = "managed-datagram-runtime")]
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
