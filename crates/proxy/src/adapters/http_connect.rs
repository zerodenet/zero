use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::EngineError;
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::protocol_adapter::{BoundInbound, InboundAdapterContext, ProtocolAdapter};

#[cfg(feature = "http_connect")]
mod inbound;

#[cfg(feature = "http_connect")]
#[derive(Debug)]
pub(crate) struct HttpConnectAdapter;

#[cfg(feature = "http_connect")]
impl ProtocolAdapter for HttpConnectAdapter {
    fn name(&self) -> &'static str {
        "http_connect"
    }

    fn feature_name(&self) -> &'static str {
        "http_connect"
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::HttpConnect)
    }

    fn supports_outbound(&self, _: &OutboundProtocolConfig) -> bool {
        false
    }

    fn has_inbound(&self) -> bool {
        true
    }

    fn has_outbound(&self) -> bool {
        false
    }

    fn spawn_inbound(
        &self,
        ctx: InboundAdapterContext<'_>,
        inbound: InboundConfig,
        bound: BoundInbound,
        shutdown_rx: tokio::sync::watch::Receiver<bool>,
        listeners: &mut tokio::task::JoinSet<Result<(), EngineError>>,
    ) {
        self.spawn_inbound_impl(ctx.proxy(), inbound, bound, shutdown_rx, listeners);
    }
}

#[cfg(feature = "http_connect")]
impl ProtocolMetadata for HttpConnectAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::http_connect::HttpConnectProtocol.descriptor()
    }
}
