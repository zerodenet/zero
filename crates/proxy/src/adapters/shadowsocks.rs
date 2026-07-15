use ::shadowsocks::transport::{ShadowsocksOutboundOptionsRef, ShadowsocksTransportLeaf};
use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::{
    named_protocol_supports_inbound, named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    ClaimedTcpOutboundLeaf, ClaimedUdpFlowLeaf, ClaimedUdpPacketPathLeaf,
    InboundListenerCapability, ManagedUdpHandlerProvider, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;

#[cfg(feature = "shadowsocks")]
mod inbound;
#[cfg(feature = "shadowsocks")]
mod tcp;
#[cfg(feature = "shadowsocks")]
pub(crate) mod udp;

#[cfg(feature = "shadowsocks")]
#[derive(Debug)]
pub(crate) struct ShadowsocksAdapter;

fn transport_leaf(leaf: &ResolvedLeafOutbound<'_>) -> Option<ShadowsocksTransportLeaf> {
    let ResolvedLeafOutbound::Shadowsocks {
        tag,
        server,
        port,
        password,
        cipher,
    } = leaf
    else {
        return None;
    };
    Some(ShadowsocksTransportLeaf::from_options_refs(
        tag,
        server,
        *port,
        ShadowsocksOutboundOptionsRef { cipher, password },
    ))
}

#[cfg(feature = "shadowsocks")]
impl NamedProtocolAdapter for ShadowsocksAdapter {
    const PROTOCOL_NAME: &'static str = "shadowsocks";
    const FEATURE_NAME: &'static str = "shadowsocks";
}

#[cfg(feature = "shadowsocks")]
impl UdpPacketPathCapability for ShadowsocksAdapter {
    fn claim_udp_packet_path_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>> {
        self.claim_udp_packet_path_leaf_impl(leaf)
    }
}

#[cfg(feature = "shadowsocks")]
impl UdpFlowCapability for ShadowsocksAdapter {
    fn claim_udp_flow_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
        self.claim_udp_flow_leaf_impl(leaf)
    }
}

#[cfg(feature = "shadowsocks")]
impl ManagedUdpHandlerProvider for ShadowsocksAdapter {
    fn managed_datagram_udp_handler(&self) -> Option<Box<dyn ManagedDatagramFlowHandler>> {
        Some(udp::managed_datagram_handler())
    }
}

#[cfg(feature = "shadowsocks")]
impl InboundListenerCapability for ShadowsocksAdapter {
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

#[cfg(feature = "shadowsocks")]
impl TcpOutboundCapability for ShadowsocksAdapter {
    fn claim_tcp_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        self.claim_tcp_outbound_leaf_impl(leaf)
    }
}

#[cfg(feature = "shadowsocks")]
impl ProtocolSupportCapability for ShadowsocksAdapter {
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

#[cfg(feature = "shadowsocks")]
impl ProtocolMetadata for ShadowsocksAdapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::shadowsocks::ShadowsocksProtocol.descriptor()
    }
}
