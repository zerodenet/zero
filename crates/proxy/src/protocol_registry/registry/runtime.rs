use super::ProtocolRegistry;

impl ProtocolRegistry {
    pub(crate) fn on_config_reloaded(&self) {
        for entry in &self.entries {
            entry.support.on_config_reloaded();
        }
    }
}
