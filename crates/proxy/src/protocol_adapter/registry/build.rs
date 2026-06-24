use super::ProtocolRegistry;
use crate::protocol_adapter::ProtocolAdapter;

impl ProtocolRegistry {
    pub(crate) fn register(&mut self, adapter: std::sync::Arc<dyn ProtocolAdapter>) {
        self.adapters.push(adapter);
    }
}
