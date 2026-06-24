use zero_config::InboundProtocolConfig;
use zero_engine::EngineError;

use tokio::sync::watch;
use tokio::task::JoinSet;

use super::ProtocolInventory;
use crate::protocol_adapter::BoundInbound;
use crate::runtime::Proxy;

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

    /// Spawn an inbound listener through its registered adapter.
    ///
    /// The runtime asks inventory to start the listener instead of resolving
    /// and holding adapter trait objects itself.
    pub(crate) fn spawn_inbound(
        &self,
        proxy: &Proxy,
        inbound: zero_config::InboundConfig,
        bound: BoundInbound,
        shutdown_rx: watch::Receiver<bool>,
        listeners: &mut JoinSet<Result<(), EngineError>>,
    ) -> Result<(), EngineError> {
        let adapter = self.registry.find_inbound(&inbound.protocol)?;
        adapter.spawn_inbound(proxy, inbound, bound, shutdown_rx, listeners);
        Ok(())
    }
}
