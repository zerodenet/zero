use std::collections::HashMap;

use super::super::super::connection::SharedManagedUdpConnection;
use super::super::key::ManagedUdpConnectionCacheKey;

pub(crate) struct ManagedUdpConnectionCache {
    pub(super) entries: HashMap<ManagedUdpConnectionCacheKey, SharedManagedUdpConnection>,
}

impl ManagedUdpConnectionCache {
    pub(crate) fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}
