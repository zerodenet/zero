//! VLESS UDP outbound manager.
//!
//! Protocol packet framing stays in `protocols/vless`; this module owns proxy
//! transport opening, cached upstream streams, metering, and response bridges.

mod bridge;
mod establish;
pub(crate) mod model;
mod send;

use crate::runtime::udp_flow::managed::ManagedStreamConnectionCache;

/// VLESS UDP outbound manager.
///
/// Response bridge tasks are spawned into the shared `chain_tasks` JoinSet
/// in [`UdpDispatch`], so all chain outbound responses are polled uniformly.
pub(crate) struct VlessUdpOutboundManager {
    upstreams: ManagedStreamConnectionCache,
}

impl VlessUdpOutboundManager {
    pub fn new() -> Self {
        Self {
            upstreams: ManagedStreamConnectionCache::new(),
        }
    }
}
