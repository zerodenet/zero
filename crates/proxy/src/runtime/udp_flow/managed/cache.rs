use std::collections::HashMap;

use super::SharedManagedUdpConnection;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ManagedUdpConnectionCacheKey(String);

impl ManagedUdpConnectionCacheKey {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }
}

pub(crate) struct ManagedUdpConnectionCache {
    entries: HashMap<ManagedUdpConnectionCacheKey, SharedManagedUdpConnection>,
}

impl ManagedUdpConnectionCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    pub(crate) fn get(
        &self,
        key: &ManagedUdpConnectionCacheKey,
    ) -> Option<&SharedManagedUdpConnection> {
        self.entries.get(key)
    }

    pub(crate) fn insert(
        &mut self,
        key: ManagedUdpConnectionCacheKey,
        value: SharedManagedUdpConnection,
    ) -> Option<SharedManagedUdpConnection> {
        self.entries.insert(key, value)
    }
}
