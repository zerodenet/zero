use std::collections::HashMap;

mod bridge;
mod establish;
pub(super) mod model;
mod send;
mod stream;

pub(crate) struct H2ChainManager {
    upstreams: HashMap<hysteria2::Hysteria2UdpCacheKey, hysteria2::Hysteria2UdpFlowSession>,
}

impl H2ChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }
}
