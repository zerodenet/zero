//! Concrete protocol capability bridges for each compiled-in protocol.

mod common;
mod direct;
#[cfg(feature = "http_connect")]
mod http_connect;
#[cfg(feature = "hysteria2")]
mod hysteria2;
#[cfg(feature = "mieru")]
mod mieru;
#[cfg(feature = "mixed")]
mod mixed;
#[cfg(feature = "shadowsocks")]
mod shadowsocks;
#[cfg(feature = "socks5")]
mod socks5;
#[cfg(feature = "trojan")]
mod trojan;
#[cfg(feature = "vless")]
mod vless;
#[cfg(feature = "vmess")]
mod vmess;

pub(crate) use direct::DirectAdapter;
#[cfg(feature = "http_connect")]
pub(crate) use http_connect::HttpConnectAdapter;
#[cfg(feature = "hysteria2")]
pub(crate) use hysteria2::udp::managed_datagram_handler as hysteria2_udp_datagram_handler;
#[cfg(feature = "hysteria2")]
pub(crate) use hysteria2::Hysteria2Adapter;
#[cfg(feature = "mieru")]
pub(crate) use mieru::udp::managed_stream_handler as mieru_udp_stream_handler;
#[cfg(feature = "mieru")]
pub(crate) use mieru::MieruAdapter;
#[cfg(feature = "mixed")]
pub(crate) use mixed::MixedAdapter;
#[cfg(feature = "shadowsocks")]
pub(crate) use shadowsocks::udp::managed_datagram_handler as shadowsocks_udp_datagram_handler;
#[cfg(feature = "shadowsocks")]
pub(crate) use shadowsocks::ShadowsocksAdapter;
#[cfg(feature = "socks5")]
pub(crate) use socks5::udp::upstream_association_handler as socks5_upstream_association_handler;
#[cfg(feature = "socks5")]
pub(crate) use socks5::Socks5Adapter;
#[cfg(feature = "trojan")]
pub(crate) use trojan::udp::managed_stream_handler as trojan_udp_stream_handler;
#[cfg(feature = "trojan")]
pub(crate) use trojan::TrojanAdapter;
#[cfg(feature = "vless")]
pub(crate) use vless::VlessAdapter;
#[cfg(feature = "vmess")]
pub(crate) use vmess::VmessAdapter;
