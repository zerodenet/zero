use zero_engine::EngineError;

use super::ProtocolInventory;
use crate::protocol_registry::OutboundLeafRuntime;

impl ProtocolInventory {
    pub(crate) fn on_config_reloaded(&self) {
        self.registry.on_config_reloaded();
    }

    /// Return the runtime-neutral facts for a resolved outbound leaf.
    ///
    /// The runtime asks the inventory for this instead of matching concrete
    /// protocol variants.
    pub(crate) fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &zero_engine::ResolvedLeafOutbound<'a>,
    ) -> Result<OutboundLeafRuntime<'a>, EngineError> {
        self.registry.outbound_leaf_runtime(leaf)
    }
}
