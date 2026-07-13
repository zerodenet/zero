#[cfg(feature = "shadowsocks")]
mod datagram;
mod key;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru",
    feature = "hysteria2"
))]
mod stream;

#[cfg(feature = "shadowsocks")]
pub(crate) use datagram::ManagedDatagramConnectionCache;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru",
    feature = "hysteria2"
))]
pub(crate) use stream::ManagedUdpConnectionCache;
