use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::EngineError;
use zero_traits::{
    ProtocolCapabilityDescriptor, ProtocolCapabilityLevel, ProtocolCapabilityState,
    ProtocolMetadata, ProtocolNetworkCapability,
};

use crate::adapters::identity::{
    named_protocol_supports_inbound, named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    InboundListenerCapability, ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability,
    UdpPacketPathCapability,
};

#[cfg(feature = "mixed")]
mod inbound;

#[cfg(feature = "mixed")]
#[derive(Debug)]
pub(crate) struct MixedAdapter;

#[cfg(feature = "mixed")]
impl NamedProtocolAdapter for MixedAdapter {
    const PROTOCOL_NAME: &'static str = "mixed";
    const FEATURE_NAME: &'static str = "mixed";
    const HAS_OUTBOUND: bool = false;
}

#[cfg(feature = "mixed")]
impl InboundListenerCapability for MixedAdapter {
    fn prepare_inbound_listener(
        &self,
        inbound: InboundConfig,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::inbound_operation::PreparedInboundListenerOperation>,
        EngineError,
    > {
        self.prepare_inbound_listener_impl(inbound)
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

#[cfg(feature = "mixed")]
impl ProtocolMetadata for MixedAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ProtocolCapabilityDescriptor {
            protocol: "mixed",
            feature: "mixed",
            status: ProtocolCapabilityLevel::Supported,
            compatibility_baseline: "kernel_builtin",
            inbound: ProtocolNetworkCapability::new(
                ProtocolCapabilityState::supported(),
                ProtocolCapabilityState::supported(),
            ),
            outbound: ProtocolNetworkCapability::new(
                ProtocolCapabilityState::unsupported(&[]),
                ProtocolCapabilityState::unsupported(&[]),
            ),
            transports: &["tcp"],
            mux: ProtocolCapabilityState::not_applicable(),
            limitations: &[],
        }
    }
}
