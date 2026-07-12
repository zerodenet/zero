#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ManagedUdpConnectionCacheKey(String);

impl ManagedUdpConnectionCacheKey {
    pub(super) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct ManagedDatagramConnectionCacheKey(String);

impl ManagedDatagramConnectionCacheKey {
    pub(super) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}
