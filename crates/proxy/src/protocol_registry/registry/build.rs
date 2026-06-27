use super::ProtocolRegistry;
use crate::protocol_registry::RegisteredProtocolCapability;

impl ProtocolRegistry {
    pub(crate) fn register<T>(&mut self, adapter: std::sync::Arc<T>)
    where
        T: RegisteredProtocolCapability + 'static,
    {
        let adapter: std::sync::Arc<dyn RegisteredProtocolCapability> = adapter;
        self.adapters.push(adapter);
    }
}
