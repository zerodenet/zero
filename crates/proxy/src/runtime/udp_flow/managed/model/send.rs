//! Managed UDP existing-send model facade.
//!
//! The root stays as a facade so datagram, stream-packet, and relay-stream send
//! models do not regrow into one mixed implementation bucket.

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
mod datagram;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
mod relay;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
mod stream;

#[cfg(any(feature = "hysteria2", feature = "shadowsocks"))]
pub(crate) use datagram::ManagedDatagramExistingSend;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) use relay::ManagedRelayExistingSend;
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru"
))]
pub(crate) use stream::ManagedStreamExistingSend;
