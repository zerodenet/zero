#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
pub(super) struct ManagedUdpConnectionCacheKey(String);

#[cfg(any(
    feature = "managed-stream-runtime",
    feature = "managed-datagram-runtime"
))]
impl ManagedUdpConnectionCacheKey {
    pub(super) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg(feature = "managed-datagram-runtime")]
pub(super) struct ManagedDatagramConnectionCacheKey(String);

#[cfg(feature = "managed-datagram-runtime")]
impl ManagedDatagramConnectionCacheKey {
    pub(super) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}
