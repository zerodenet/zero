mod fixtures;
mod inbound;
mod outbound;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
mod registration;
mod validation;

pub(crate) use fixtures::fake_direct_leaf;
