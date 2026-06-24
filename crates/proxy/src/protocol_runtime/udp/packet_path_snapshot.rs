/// Carrier parameters for a UDP packet path relay chain hop.
///
/// Stores the connection parameters for the packet path provider so that an
/// existing flow can re-dispatch packets through the same carrier without
/// re-resolving the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UdpPacketPathCarrier {
    #[cfg(feature = "socks5")]
    Socks5 {
        cache_key: String,
        tag: String,
        server: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    #[cfg(feature = "shadowsocks")]
    Shadowsocks {
        cache_key: String,
        tag: String,
        server: String,
        port: u16,
        password: String,
    },
    #[cfg(feature = "hysteria2")]
    Hysteria2 {
        cache_key: String,
        tag: String,
        server: String,
        port: u16,
        password: String,
        client_fingerprint: Option<String>,
    },
}

impl UdpPacketPathCarrier {
    /// Cache key matching the one the adapter produced at flow-start time, so
    /// the packet-path manager can re-find the cached carrier on forward.
    pub(crate) fn cache_key(&self) -> &str {
        match self {
            #[cfg(feature = "socks5")]
            Self::Socks5 { cache_key, .. } => cache_key,
            #[cfg(feature = "shadowsocks")]
            Self::Shadowsocks { cache_key, .. } => cache_key,
            #[cfg(feature = "hysteria2")]
            Self::Hysteria2 { cache_key, .. } => cache_key,
        }
    }
}

pub(crate) fn socks5_packet_path_carrier_descriptor(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
) -> crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
    crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
        cache_key: crate::protocol_runtime::udp::socks5_udp_cache_key(tag, server, port, username),
        server: server.to_owned(),
        port,
    }
}

#[cfg(feature = "socks5")]
pub(crate) fn socks5_packet_path_carrier_snapshot(
    tag: &str,
    server: &str,
    port: u16,
    username: Option<&str>,
    password: Option<&str>,
) -> UdpPacketPathCarrier {
    UdpPacketPathCarrier::Socks5 {
        cache_key: crate::protocol_runtime::udp::socks5_udp_cache_key(tag, server, port, username),
        tag: tag.to_owned(),
        server: server.to_owned(),
        port,
        username: username.map(str::to_owned),
        password: password.map(str::to_owned),
    }
}

pub(crate) fn shadowsocks_packet_path_carrier_descriptor(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
    crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
        cache_key: crate::protocol_runtime::udp::shadowsocks_udp_cache_key(
            tag, server, port, cipher, password,
        ),
        server: server.to_owned(),
        port,
    }
}

#[cfg(feature = "shadowsocks")]
pub(crate) fn shadowsocks_packet_path_carrier_snapshot(
    tag: &str,
    server: &str,
    port: u16,
    cipher: &str,
    password: &str,
) -> UdpPacketPathCarrier {
    UdpPacketPathCarrier::Shadowsocks {
        cache_key: crate::protocol_runtime::udp::shadowsocks_udp_cache_key(
            tag, server, port, cipher, password,
        ),
        tag: tag.to_owned(),
        server: server.to_owned(),
        port,
        password: password.to_owned(),
    }
}

#[cfg(feature = "hysteria2")]
pub(crate) fn hysteria2_packet_path_carrier_descriptor(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
    crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
        cache_key: crate::protocol_runtime::udp::hysteria2_udp_cache_key(
            tag,
            server,
            port,
            password,
            client_fingerprint,
        ),
        server: server.to_owned(),
        port,
    }
}

#[cfg(feature = "hysteria2")]
pub(crate) fn hysteria2_packet_path_carrier_snapshot(
    tag: &str,
    server: &str,
    port: u16,
    password: &str,
    client_fingerprint: Option<&str>,
) -> UdpPacketPathCarrier {
    UdpPacketPathCarrier::Hysteria2 {
        cache_key: crate::protocol_runtime::udp::hysteria2_udp_cache_key(
            tag,
            server,
            port,
            password,
            client_fingerprint,
        ),
        tag: tag.to_owned(),
        server: server.to_owned(),
        port,
        password: password.to_owned(),
        client_fingerprint: client_fingerprint.map(str::to_owned),
    }
}
