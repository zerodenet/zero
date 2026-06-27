mod connect;
#[cfg(feature = "trojan")]
mod establish;
#[cfg(feature = "trojan")]
pub(super) mod model;
#[cfg(feature = "trojan")]
mod send;

#[cfg(feature = "trojan")]
pub(crate) struct TrojanChainManager {
    upstreams: trojan::TrojanUdpFlowSessions,
}

#[cfg(feature = "trojan")]
impl TrojanChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: trojan::TrojanUdpFlowSessions::new(),
        }
    }
}
