#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use zero_engine::SessionOutcome;

use super::super::model::{UdpFlowCompletion, UdpFlowOutbound};

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
    fn upstream_endpoint(&self) -> Option<(String, u16)> {
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
            Self::PacketPathDatagram { server, port, .. } => Some((server.clone(), *port)),
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { server, port, .. } => Some((server.clone(), *port)),
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            Self::Datagram { server, port, .. } => Some((server.clone(), *port)),
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { server, port, .. } => Some((server.clone(), *port)),
        }
    }

    fn success_outcome(&self) -> SessionOutcome {
        match self {
            Self::Direct { .. } => SessionOutcome::DirectRelayed,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "hysteria2",
                feature = "shadowsocks",
                feature = "trojan",
                feature = "vmess",
                feature = "mieru"
            ))]
            Self::PacketPathDatagram { .. } => SessionOutcome::ChainedRelayed,
            #[cfg(any(
                feature = "socks5",
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::Relay { .. } => SessionOutcome::ChainedRelayed,
            #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
            Self::Datagram { .. } => SessionOutcome::ChainedRelayed,
            #[cfg(any(
                feature = "vless",
                feature = "vmess",
                feature = "trojan",
                feature = "mieru"
            ))]
            Self::StreamPacket { .. } => SessionOutcome::ChainedRelayed,
        }
    }

    pub(in crate::runtime::udp_flow) fn completion(&self) -> UdpFlowCompletion {
        UdpFlowCompletion {
            upstream: self.upstream_endpoint(),
            success_outcome: self.success_outcome(),
        }
    }
}
