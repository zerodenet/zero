mod contract;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod runtime;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod state;

pub(crate) use contract::UpstreamAssociationHandler;
pub(crate) use contract::UpstreamAssociationSend;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use contract::UpstreamUdpHandlers;
#[cfg(feature = "socks5")]
pub(crate) use contract::{
    UpstreamAssociationCloseReason, UpstreamAssociationStages, UpstreamAssociationTarget,
    UpstreamAssociationTransport,
};
#[cfg(feature = "socks5")]
pub(crate) use runtime::boxed_registered_upstream_handler;
#[cfg(all(test, feature = "socks5"))]
pub(crate) use runtime::UpstreamAssociationRuntime;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(super) use state::handlers::UpstreamAssociationState;
#[cfg(all(test, feature = "socks5"))]
pub(crate) use state::TrackedUpstreamAssociationState;

#[cfg(all(test, feature = "socks5"))]
mod tests;
