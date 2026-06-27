use std::collections::HashMap;

mod bridge;
mod establish;
pub(super) mod model;
mod send;
mod stream;

use model::H2Entry;

pub(crate) struct H2ChainManager {
    upstreams: HashMap<hysteria2::Hysteria2UdpCacheKey, H2Entry>,
}

impl H2ChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }
}
