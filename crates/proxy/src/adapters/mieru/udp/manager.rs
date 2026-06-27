#[cfg(feature = "mieru")]
mod connect;
#[cfg(feature = "mieru")]
mod establish;
#[cfg(feature = "mieru")]
pub(super) mod model;
#[cfg(feature = "mieru")]
mod send;
#[cfg(feature = "mieru")]
mod stream;

#[cfg(feature = "mieru")]
pub(crate) struct MieruChainManager {
    upstreams: mieru::MieruUdpFlowStore<mieru::MieruUdpFlowSession>,
}

#[cfg(feature = "mieru")]
impl MieruChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: mieru::MieruUdpFlowStore::new(),
        }
    }
}
