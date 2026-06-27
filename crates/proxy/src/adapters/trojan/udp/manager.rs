#[cfg(feature = "trojan")]
use std::collections::HashMap;

#[cfg(feature = "trojan")]
mod bridge;
#[cfg(feature = "trojan")]
mod connect;
#[cfg(feature = "trojan")]
mod establish;
#[cfg(feature = "trojan")]
pub(super) mod model;
#[cfg(feature = "trojan")]
mod send;
#[cfg(feature = "trojan")]
mod stream;

#[cfg(feature = "trojan")]
pub(crate) struct TrojanChainManager {
    upstreams: HashMap<trojan::TrojanUdpCacheKey, trojan::TrojanUdpFlowSession>,
}

#[cfg(feature = "trojan")]
impl TrojanChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }
}
