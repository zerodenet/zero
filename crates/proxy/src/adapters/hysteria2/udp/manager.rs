mod establish;
pub(super) mod model;
mod send;

pub(crate) struct H2ChainManager {
    upstreams: hysteria2::Hysteria2UdpFlowStore<hysteria2::Hysteria2UdpFlowSession>,
}

impl H2ChainManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: hysteria2::Hysteria2UdpFlowStore::new(),
        }
    }
}
