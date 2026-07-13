use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::EngineError;
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::{
    named_protocol_supports_inbound, named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    BoundInbound, InboundAdapterContext, InboundListenerCapability, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};

#[cfg(feature = "http_connect")]
pub(super) mod inbound;

#[cfg(feature = "http_connect")]
#[derive(Debug)]
pub(crate) struct HttpConnectAdapter;

#[cfg(feature = "http_connect")]
impl NamedProtocolAdapter for HttpConnectAdapter {
    const PROTOCOL_NAME: &'static str = "http_connect";
    const FEATURE_NAME: &'static str = "http_connect";
    const HAS_OUTBOUND: bool = false;
}

#[cfg(feature = "http_connect")]
impl InboundListenerCapability for HttpConnectAdapter {
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
impl TcpOutboundCapability for HttpConnectAdapter {}

#[cfg(feature = "http_connect")]
impl UdpFlowCapability for HttpConnectAdapter {}

#[cfg(feature = "http_connect")]
impl UdpPacketPathCapability for HttpConnectAdapter {}

#[cfg(feature = "http_connect")]
impl ProtocolSupportCapability for HttpConnectAdapter {
    fn name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::PROTOCOL_NAME
    }

    fn feature_name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::FEATURE_NAME
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        named_protocol_supports_inbound::<Self>(c)
    }

    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        named_protocol_supports_outbound::<Self>(c)
    }

    fn has_inbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_INBOUND
    }

    fn has_outbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_OUTBOUND
    }
}

#[cfg(feature = "http_connect")]
impl ProtocolMetadata for HttpConnectAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::http_connect::HttpConnectProtocol.descriptor()
    }
}
