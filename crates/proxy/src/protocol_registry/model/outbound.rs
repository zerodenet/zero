use crate::runtime::path::{OutboundEndpoint, TcpPathCategory};

/// Runtime-neutral facts about one resolved outbound leaf.
///
/// The proxy runtime uses this for orchestration decisions without matching on
/// concrete protocol variants. Protocol-private fields remain owned by the
/// adapter that claimed the leaf.
#[derive(Debug, Clone)]
pub(crate) struct OutboundLeafRuntime {
    pub(crate) tcp_path: TcpPathCategory,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) health_tag: Option<String>,
    pub(crate) endpoint: Option<OutboundEndpoint>,
    pub(crate) kernel_tag: Option<String>,
    #[cfg(any(
        feature = "socks5",
        feature = "vless",
        feature = "hysteria2",
        feature = "shadowsocks",
        feature = "trojan",
        feature = "vmess",
        feature = "mieru"
    ))]
    pub(crate) udp_policy_tag: Option<String>,
}
