use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

use super::ProtocolInventory;
use crate::protocol_registry::{BoundInbound, InboundListenerCapability};
use crate::runtime::inbound_operation::PreparedInboundListenerOperation;

impl ProtocolInventory {
    pub(crate) fn check_inbound_enabled(
        &self,
        protocol: &InboundProtocolConfig,
        tag: &str,
    ) -> Result<(), EngineError> {
        if self.registry.supports_inbound(protocol) {
            return Ok(());
        }
        let label = self.registry.inbound_protocol_label(protocol);
        let feature = self.registry.inbound_protocol_feature_name(protocol);
        Err(EngineError::CompiledFeatureDisabled {
            kind: "inbound",
            tag: tag.to_owned(),
            protocol: label,
            feature,
        })
    }

    pub(crate) async fn bind_inbound(
        &self,
        inbound: &zero_config::InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        self.registry.bind_inbound(inbound, source_dir).await
    }

    /// Resolve the registered adapter and prepare its listener operation.
    pub(crate) fn prepare_inbound_listener(
        &self,
        inbound: zero_config::InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedInboundListenerOperation>, EngineError> {
        let adapter = self.registry.find_inbound(&inbound.protocol)?;
        InboundListenerCapability::prepare_inbound_listener(adapter.as_ref(), inbound, source_dir)
    }
}
