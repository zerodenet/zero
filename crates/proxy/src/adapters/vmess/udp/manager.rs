//! VMess UDP upstream manager.
//!
//! VMess protocol state stays in `protocols/vmess`; this module only handles
//! dialing transports, caching per-target upstream streams, metering, and
//! response bridge tasks.

mod bridge;
mod establish;
pub(crate) mod model;
mod send;

use std::collections::HashMap;

use zero_core::Address;

use model::VmessUdpUpstream;

pub(crate) struct VmessUdpOutboundManager {
    upstreams: HashMap<(Address, u16), VmessUdpUpstream>,
}

impl VmessUdpOutboundManager {
    pub(crate) fn new() -> Self {
        Self {
            upstreams: HashMap::new(),
        }
    }
}
