use crate::runtime::orchestration::OutboundEndpoint;

pub(crate) type UdpPeerEndpoint<'a> = OutboundEndpoint<'a>;

/// Shadowsocks UDP peer parameters.
#[cfg(feature = "shadowsocks")]
pub(crate) struct SsUdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) cache_key: &'a str,
}

/// Hysteria2 UDP peer parameters.
#[cfg(feature = "hysteria2")]
pub(crate) struct H2UdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) password: &'a str,
    pub(crate) client_fingerprint: Option<&'a str>,
}

/// Trojan UDP peer parameters.
#[cfg(feature = "trojan")]
pub(crate) struct TrojanUdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) password: &'a str,
    pub(crate) sni: Option<&'a str>,
    pub(crate) insecure: bool,
    pub(crate) client_fingerprint: Option<&'a str>,
    pub(crate) relay_chain: bool,
}

/// Mieru UDP peer parameters.
#[cfg(feature = "mieru")]
pub(crate) struct MieruUdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) username: &'a str,
    pub(crate) password: &'a str,
    pub(crate) relay_chain: bool,
}
