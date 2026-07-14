use zero_engine::EngineError;

use super::ProtocolInventory;
use crate::protocol_registry::ClaimedOutboundLeaf;

impl ProtocolInventory {
    pub(crate) fn on_config_reloaded(&self) {
        self.registry.on_config_reloaded();
    }

    pub(crate) fn claim_outbound_leaf<'a>(
        &self,
        leaf: &zero_engine::ResolvedLeafOutbound<'a>,
    ) -> Result<ClaimedOutboundLeaf<'a>, EngineError> {
        self.registry.claim_outbound_leaf(leaf)
    }
}
