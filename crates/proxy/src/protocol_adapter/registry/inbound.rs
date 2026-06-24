use std::sync::Arc;

use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

use super::ProtocolRegistry;
use crate::protocol_adapter::{BoundInbound, ProtocolAdapter};

impl ProtocolRegistry {
    /// Find the adapter that handles this inbound config, if any.
    ///
    /// Single dispatch point: the runtime resolves an inbound config to its
    /// adapter here instead of matching on the protocol enum.
    pub(crate) fn find_inbound(
        &self,
        config: &InboundProtocolConfig,
    ) -> Result<Arc<dyn ProtocolAdapter>, EngineError> {
        for adapter in &self.adapters {
            if adapter.supports_inbound(config) {
                return Ok(adapter.clone());
            }
        }
        let name = self.inbound_protocol_label(config);
        Err(EngineError::CompiledFeatureDisabled {
            kind: "inbound",
            tag: String::new(),
            protocol: name,
            feature: self.inbound_protocol_feature_name(config),
        })
    }

    /// Bind an inbound listener via its registered adapter.
    ///
    /// Single dispatch point: the runtime resolves an inbound config to its
    /// adapter and binds the socket here, instead of matching on the protocol
    /// enum. Port conflicts surface before the accept loop spawns.
    pub(crate) async fn bind_inbound(
        &self,
        inbound: &zero_config::InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        for adapter in &self.adapters {
            if adapter.supports_inbound(&inbound.protocol) {
                return adapter.bind_inbound(inbound, source_dir).await;
            }
        }
        let name = self.inbound_protocol_label(&inbound.protocol);
        Err(EngineError::CompiledFeatureDisabled {
            kind: "inbound",
            tag: inbound.tag.clone(),
            protocol: name,
            feature: self.inbound_protocol_feature_name(&inbound.protocol),
        })
    }
}
