use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::EngineError;
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::protocol_capability::protocol_descriptor;
use crate::protocol_registry::{
    BoundInbound, InboundAdapterContext, InboundListenerCapability, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};

#[cfg(feature = "mixed")]
mod inbound;

#[cfg(feature = "mixed")]
#[derive(Debug)]
pub(crate) struct MixedAdapter;

#[cfg(feature = "mixed")]
impl InboundListenerCapability for MixedAdapter {
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

#[cfg(feature = "mixed")]
impl TcpOutboundCapability for MixedAdapter {}

#[cfg(feature = "mixed")]
impl UdpFlowCapability for MixedAdapter {}

#[cfg(feature = "mixed")]
impl UdpPacketPathCapability for MixedAdapter {}

#[cfg(feature = "mixed")]
impl ProtocolSupportCapability for MixedAdapter {
    fn name(&self) -> &'static str {
        "mixed"
    }

    fn feature_name(&self) -> &'static str {
        "mixed"
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        matches!(c, InboundProtocolConfig::Mixed { .. })
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
}

#[cfg(feature = "mixed")]
impl ProtocolMetadata for MixedAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        protocol_descriptor("mixed", "mixed")
    }
}
