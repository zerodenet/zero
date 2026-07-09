use super::ProtocolRegistry;
use crate::protocol_registry::RegisteredProtocolCapability;

impl ProtocolRegistry {
    pub(crate) fn register_capability(
        &mut self,
        adapter: std::sync::Arc<dyn RegisteredProtocolCapability>,
    ) {
        self.adapters.push(adapter);
    }
}
