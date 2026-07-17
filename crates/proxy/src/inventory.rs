use crate::protocol_registry::ProtocolRegistry;

mod inbound;
mod metadata;
mod protocols;
mod runtime;
mod tcp;
#[cfg(test)]
mod tests;
#[cfg(feature = "udp-runtime")]
mod udp;

pub(crate) use runtime::{ClaimedInventoryLeaf, ClaimedRelayChain};
pub(crate) use tcp::PreparedTcpRelayChain;
pub(crate) use tcp::{
    PreparedTcpCandidate, PreparedTcpCandidateExecution, PreparedTcpOutbound, PreparedTcpRelayHop,
};
#[cfg(feature = "udp-runtime")]
pub(crate) use udp::{PreparedUdpLeafCandidate, PreparedUdpOutbound};

#[derive(Debug, Clone)]
pub struct ProtocolInventory {
    registry: ProtocolRegistry,
}

impl Default for ProtocolInventory {
    fn default() -> Self {
        Self {
            registry: crate::register::protocol_registry(),
        }
    }
}

#[cfg(feature = "udp-runtime")]

impl ProtocolInventory {
    pub(crate) fn registered_udp_handlers(
        &self,
    ) -> crate::runtime::udp_flow::registered::RegisteredUdpHandlers {
        crate::register::registered_udp_handlers(&self.registry)
    }
}
