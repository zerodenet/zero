#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru",
    feature = "hysteria2"
))]
pub(super) struct ManagedUdpConnectionCacheKey(String);

#[cfg(any(
    feature = "vless",
    feature = "vmess",
    feature = "trojan",
    feature = "mieru",
    feature = "hysteria2"
))]
impl ManagedUdpConnectionCacheKey {
    pub(super) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg(feature = "shadowsocks")]
pub(super) struct ManagedDatagramConnectionCacheKey(String);

#[cfg(feature = "shadowsocks")]
impl ManagedDatagramConnectionCacheKey {
    pub(super) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}
