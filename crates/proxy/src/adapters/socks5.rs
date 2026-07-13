use async_trait::async_trait;

use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    proxy_leaf_runtime, InboundListenerCapability, OutboundLeafRuntime, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability, UpstreamUdpHandlerProvider,
};
use crate::runtime::path::TcpPathCategory;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::registered::UpstreamAssociationHandler;
use crate::transport::TcpOutboundFailure;

#[cfg(feature = "socks5")]
pub(super) mod inbound;
#[cfg(feature = "socks5")]
mod tcp;
#[cfg(feature = "socks5")]
pub(crate) mod udp;

#[cfg(feature = "socks5")]
#[derive(Debug)]
pub(crate) struct Socks5Adapter;

#[cfg(feature = "socks5")]
impl NamedProtocolAdapter for Socks5Adapter {
    const PROTOCOL_NAME: &'static str = "socks5";
    const FEATURE_NAME: &'static str = "socks5";
}

#[cfg(feature = "socks5")]
impl UdpPacketPathCapability for Socks5Adapter {
    fn prepare_udp_packet_path<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Option<
        Box<
            dyn crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation
                + 'a,
        >,
    > {
        self.prepare_udp_packet_path_impl(leaf)
    }
}

#[cfg(feature = "socks5")]
#[async_trait]
impl UdpFlowCapability for Socks5Adapter {
    fn prepare_udp_flow<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<
        Box<dyn crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation + 'a>,
        FlowFailure,
    > {
        self.prepare_udp_flow_impl(leaf)
    }
}

#[cfg(feature = "socks5")]
impl UpstreamUdpHandlerProvider for Socks5Adapter {
    fn upstream_association_handler(&self) -> Box<dyn UpstreamAssociationHandler> {
        udp::upstream_association_handler()
    }
}

#[cfg(feature = "socks5")]
impl InboundListenerCapability for Socks5Adapter {
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

#[cfg(feature = "socks5")]
#[async_trait]
impl TcpOutboundCapability for Socks5Adapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        named_protocol_claims_runtime_leaf::<Self>(leaf)
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        proxy_leaf_runtime(leaf, TcpPathCategory::Tunnel)
    }

    fn prepare_tcp_connect<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpConnectOperation + 'a>,
        TcpOutboundFailure,
    > {
        self.prepare_tcp_connect_impl(leaf)
    }

    fn prepare_tcp_relay_hop<'a>(
        &'a self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpRelayOperation + 'a>,
        EngineError,
    > {
        self.prepare_tcp_relay_hop_impl(leaf)
    }
}

#[cfg(feature = "socks5")]
impl ProtocolSupportCapability for Socks5Adapter {
    fn name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::PROTOCOL_NAME
    }

    fn feature_name(&self) -> &'static str {
        <Self as NamedProtocolAdapter>::FEATURE_NAME
    }

    fn has_inbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_INBOUND
    }

    fn has_outbound(&self) -> bool {
        <Self as NamedProtocolAdapter>::HAS_OUTBOUND
    }

    fn supports_inbound(&self, c: &InboundProtocolConfig) -> bool {
        named_protocol_supports_inbound::<Self>(c)
    }

    fn supports_outbound(&self, c: &OutboundProtocolConfig) -> bool {
        named_protocol_supports_outbound::<Self>(c)
    }
}

#[cfg(feature = "socks5")]
impl ProtocolMetadata for Socks5Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::socks5::Socks5Protocol.descriptor()
    }
}
