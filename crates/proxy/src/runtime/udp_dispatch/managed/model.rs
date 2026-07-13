#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use crate::runtime::udp_flow::managed::{ManagedUdpFlowKind, ManagedUdpFlowResume};
use crate::runtime::Proxy;
use zero_core::Session;

#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(super) struct ManagedUdpSend<'a> {
    pub(super) proxy: Option<&'a Proxy>,
    #[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
    pub(super) tag: &'a str,
    pub(super) session: &'a Session,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(super) carrier: Option<crate::transport::RelayCarrier>,
    #[cfg(any(
        feature = "vless",
        feature = "vmess",
        feature = "trojan",
        feature = "mieru"
    ))]
    pub(super) tls_server_name: Option<&'a str>,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: ManagedUdpFlowResume,
    pub(super) payload: &'a [u8],
    pub(super) kind: ManagedUdpFlowKind,
}

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) struct ManagedDatagramStart<'a, T> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: T,
    pub(crate) payload: &'a [u8],
}

#[cfg(feature = "socks5")]
pub(crate) struct UpstreamTrackedStart<'a, T> {
    pub(crate) proxy: Option<&'a Proxy>,
    pub(crate) tag: &'a str,
    pub(crate) session: &'a Session,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: T,
    pub(crate) payload: &'a [u8],
}
