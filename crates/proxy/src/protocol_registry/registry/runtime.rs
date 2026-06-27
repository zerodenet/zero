use super::ProtocolRegistry;

impl ProtocolRegistry {
    pub(crate) fn on_config_reloaded(&self) {
        for adapter in &self.adapters {
            adapter.on_config_reloaded();
        }
    }
}
