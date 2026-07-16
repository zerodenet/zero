//! Managed cache insert facade.
//!
//! The root stays as a facade so pre-sent cache reuse, establish-on-miss, and
//! relay insert-and-send paths do not regrow into one mixed implementation
//! bucket.

mod establish;
#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
mod pre_sent;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
mod relay;
