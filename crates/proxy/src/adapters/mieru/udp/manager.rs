#[cfg(feature = "mieru")]
mod connect;
#[cfg(feature = "mieru")]
mod establish;
#[cfg(feature = "mieru")]
pub(super) mod model;
#[cfg(feature = "mieru")]
mod send;

#[cfg(feature = "mieru")]
use crate::runtime::udp_flow::managed::ManagedUdpConnectionCache;

#[cfg(feature = "mieru")]
pub(crate) struct MieruChainManager {
    upstreams: ManagedUdpConnectionCache,
}

#[cfg(feature = "mieru")]
impl MieruChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: ManagedUdpConnectionCache::new(),
        }
    }
}
