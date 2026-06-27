mod establish;
pub(super) mod model;
mod send;

use crate::runtime::udp_flow::managed::ManagedUdpConnectionCache;

pub(crate) struct H2ChainManager {
    upstreams: ManagedUdpConnectionCache,
}

impl H2ChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: ManagedUdpConnectionCache::new(),
        }
    }
}
