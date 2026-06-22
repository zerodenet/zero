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
        cipher: String,
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
