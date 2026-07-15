use zero_core::Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OutboundEndpoint {
    pub(crate) server: String,
    pub(crate) port: u16,
}

impl OutboundEndpoint {
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) fn upstream(&self) -> (String, u16) {
        (self.server.clone(), self.port)
    }

    pub(crate) fn address(&self) -> Address {
        Address::Domain(self.server.clone())
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UdpPathCategory {
    Direct,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    Relay,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    StreamPacket,
    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    Datagram,
    PacketPathDatagram,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TcpPathCategory {
    Direct,
    Block,
    #[cfg(any(feature = "socks5", feature = "vless", feature = "trojan"))]
    Tunnel,
    #[cfg(any(feature = "shadowsocks", feature = "vmess", feature = "mieru"))]
    Session,
    #[cfg(feature = "hysteria2")]
    TransportSession,
}
