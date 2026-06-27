//! VMess UDP upstream manager.
//!
//! VMess protocol state stays in `protocols/vmess`; this module only handles
//! dialing transports, caching per-target upstream streams, metering, and
//! response bridge tasks.

mod bridge;
mod establish;
pub(crate) mod model;
mod send;

use crate::runtime::udp_flow::managed::ManagedStreamConnectionCache;

pub(crate) struct VmessUdpOutboundManager {
    upstreams: ManagedStreamConnectionCache,
}

impl VmessUdpOutboundManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: ManagedStreamConnectionCache::new(),
        }
    }
}
