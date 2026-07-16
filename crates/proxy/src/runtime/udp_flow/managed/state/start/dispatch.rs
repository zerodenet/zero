//! Managed UDP start dispatch facade.
//!
//! The root stays as a facade so request-kind selection, datagram extraction,
//! and stream extraction do not collapse back into one implementation bucket.

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
mod datagram;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
mod relay;
mod request;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
mod stream;
