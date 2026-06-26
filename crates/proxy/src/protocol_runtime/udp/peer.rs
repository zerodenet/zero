use crate::runtime::orchestration::OutboundEndpoint;

pub(crate) type UdpPeerEndpoint<'a> = OutboundEndpoint<'a>;

/// Shadowsocks UDP peer parameters.
#[cfg(feature = "shadowsocks")]
pub(crate) struct SsUdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) leaf_key: shadowsocks::ShadowsocksUdpLeafKey,
}

/// Hysteria2 UDP peer parameters.
#[cfg(feature = "hysteria2")]
pub(crate) struct H2UdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) resume: &'a hysteria2::Hysteria2UdpFlowResume,
}

/// Trojan UDP peer parameters.
#[cfg(feature = "trojan")]
pub(crate) struct TrojanUdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) resume: &'a trojan::TrojanUdpFlowResume,
    pub(crate) flow_key: trojan::TrojanUdpFlowKey,
}

/// Mieru UDP peer parameters.
#[cfg(feature = "mieru")]
pub(crate) struct MieruUdpPeer<'a> {
    pub(crate) endpoint: UdpPeerEndpoint<'a>,
    pub(crate) resume: &'a mieru::MieruUdpFlowResume,
    pub(crate) relay_chain: bool,
}
