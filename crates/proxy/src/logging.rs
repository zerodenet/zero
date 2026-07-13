//! Responsibility-grouped runtime event logging.

mod group;
#[cfg(any(
    feature = "socks5",
    feature = "http",
    feature = "mixed",
    feature = "vless",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod listener;
mod session;
#[cfg(feature = "socks5")]
mod udp_upstream;

pub(crate) use group::log_urltest_group_target_changed;
#[cfg(any(
    feature = "socks5",
    feature = "http",
    feature = "mixed",
    feature = "vless",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) use listener::{log_listener_connection_error, INBOUND_ACCEPT_ROUTE_STAGE};
pub(crate) use session::{log_session_accepted, log_session_failed, log_session_finished};
#[cfg(feature = "socks5")]
pub(crate) use udp_upstream::{
    log_udp_upstream_association_created, log_udp_upstream_association_dropped,
    log_udp_upstream_association_idle_timeout, log_udp_upstream_association_reused,
};
