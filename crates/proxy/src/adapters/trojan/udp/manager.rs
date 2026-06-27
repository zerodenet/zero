mod connect;
#[cfg(feature = "trojan")]
mod establish;
#[cfg(feature = "trojan")]
pub(super) mod model;
#[cfg(feature = "trojan")]
mod send;

#[cfg(feature = "trojan")]
use crate::runtime::udp_flow::managed::ManagedUdpConnectionCache;

#[cfg(feature = "trojan")]
pub(crate) struct TrojanChainManager {
    upstreams: ManagedUdpConnectionCache,
}

#[cfg(feature = "trojan")]
impl TrojanChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: ManagedUdpConnectionCache::new(),
        }
    }
}
