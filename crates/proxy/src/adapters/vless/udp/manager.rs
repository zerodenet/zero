//! VLESS UDP outbound manager.
//!
//! Protocol packet framing stays in `protocols/vless`; this module owns proxy
//! transport opening, cached upstream streams, metering, and response bridges.

mod bridge;
mod establish;
pub(crate) mod model;
mod send;

use std::collections::HashMap;

use zero_core::Address;

use model::VlessUdpUpstream;

/// VLESS UDP outbound manager.
///
/// Response bridge tasks are spawned into the shared `chain_tasks` JoinSet
/// in [`UdpDispatch`], so all chain outbound responses are polled uniformly.
pub(crate) struct VlessUdpOutboundManager {
    upstreams: HashMap<(Address, u16), VlessUdpUpstream>,
}

impl VlessUdpOutboundManager {
    pub fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }
}
