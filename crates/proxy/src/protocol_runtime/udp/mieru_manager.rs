#[cfg(feature = "mieru")]
use std::collections::HashMap;

#[cfg(feature = "mieru")]
mod bridge;
#[cfg(feature = "mieru")]
mod codec;
#[cfg(feature = "mieru")]
mod connect;
#[cfg(feature = "mieru")]
mod establish;
#[cfg(feature = "mieru")]
mod model;
#[cfg(feature = "mieru")]
mod send;
#[cfg(feature = "mieru")]
mod stream;

#[cfg(feature = "mieru")]
use model::{MieruEntry, MieruKey};
#[cfg(feature = "mieru")]
pub(crate) use model::{MieruRelayExisting, MieruSendExisting};

#[cfg(feature = "mieru")]
pub(crate) struct MieruChainManager {
    upstreams: HashMap<MieruKey, MieruEntry>,
}

#[cfg(feature = "mieru")]
impl MieruChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }
}
