use std::net::SocketAddr;

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

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ManagedUdpFlowRef(pub(crate) u64);

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
impl ManagedUdpFlowRef {
    pub(crate) fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Outbound type tracked per UDP flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UdpFlowOutbound {
    Direct {
        tag: String,
        target_addr: SocketAddr,
    },
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    Relay {
        tag: String,
        server: String,
        port: u16,
        managed: ManagedUdpFlowRef,
    },
    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    Datagram {
        tag: String,
        server: String,
        port: u16,
        managed: ManagedUdpFlowRef,
    },
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    StreamPacket {
        tag: String,
        server: String,
        port: u16,
        managed: ManagedUdpFlowRef,
    },
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    PacketPathDatagram {
        tag: String,
        server: String,
        port: u16,
        snapshot: crate::runtime::udp_flow::packet_path::PacketPathFlowSnapshot,
    },
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
pub(in crate::runtime::udp_flow) struct UdpFlowIndexKeys<'a> {
    pub(in crate::runtime::udp_flow) direct_sender: Option<SocketAddr>,
    pub(in crate::runtime::udp_flow) upstream_response_tag: Option<&'a str>,
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
pub(in crate::runtime::udp_flow) struct UdpFlowCompletion {
    pub(in crate::runtime::udp_flow) upstream: Option<(String, u16)>,
    pub(in crate::runtime::udp_flow) success_outcome: SessionOutcome,
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
pub(crate) struct UdpFlowUpstream<'a> {
    pub(crate) server: &'a str,
    pub(crate) port: u16,
}
