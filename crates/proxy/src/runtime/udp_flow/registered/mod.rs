//! UDP handlers registered at proxy assembly time and their neutral state.
//!
//! This layer selects and invokes registered managed-flow and upstream
//! association handlers. The reusable connection machinery lives in sibling
//! `managed`; concrete protocol state remains opaque.

#[cfg(any(
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod forward;
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
#[cfg(feature = "socks5")]
mod upstream;

#[cfg(feature = "socks5")]
pub(crate) use state::ClosedRegisteredUpstreamAssociation;
#[cfg(feature = "socks5")]
pub(crate) use state::RegisteredUpstreamAssociationView;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use state::{RegisteredUdpHandlers, RegisteredUdpState};
#[cfg(feature = "socks5")]
pub(crate) use upstream::boxed_registered_upstream_handler;
#[cfg(feature = "socks5")]
pub(crate) use upstream::UpstreamAssociationHandler;
#[cfg(feature = "socks5")]
pub(crate) use upstream::UpstreamAssociationSend;
#[cfg(any(
    test,
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
#[cfg(feature = "socks5")]
pub(crate) use upstream::UpstreamUdpHandlers;
#[cfg(feature = "socks5")]
pub(crate) use upstream::{
    UpstreamAssociationCloseReason, UpstreamAssociationStages, UpstreamAssociationTarget,
    UpstreamAssociationTransport,
};
