use crate::protocol_registry::ProtocolRegistry;

mod inbound;
mod metadata;
mod protocols;
mod runtime;
mod tcp;
mod udp;

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
