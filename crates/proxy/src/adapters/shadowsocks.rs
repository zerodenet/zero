use async_trait::async_trait;

use ::shadowsocks::transport::ShadowsocksTransportLeaf;
use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    proxy_leaf_runtime, InboundListenerCapability, ManagedUdpHandlerProvider, OutboundLeafRuntime,
    ProtocolSupportCapability, TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;
use crate::transport::TcpOutboundFailure;

#[cfg(feature = "shadowsocks")]
mod inbound;
#[cfg(feature = "shadowsocks")]
mod tcp;
#[cfg(feature = "shadowsocks")]
pub(crate) mod udp;

#[cfg(feature = "shadowsocks")]
#[derive(Debug)]
pub(crate) struct ShadowsocksAdapter;

fn transport_leaf<'a>(leaf: &'a ResolvedLeafOutbound<'a>) -> Option<ShadowsocksTransportLeaf<'a>> {
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
    Some(ShadowsocksTransportLeaf::new(
        tag, server, *port, cipher, password,
    ))
}

#[cfg(feature = "shadowsocks")]
impl NamedProtocolAdapter for ShadowsocksAdapter {
    const PROTOCOL_NAME: &'static str = "shadowsocks";
    const FEATURE_NAME: &'static str = "shadowsocks";
}

#[cfg(feature = "shadowsocks")]
impl UdpPacketPathCapability for ShadowsocksAdapter {
    fn prepare_udp_packet_path<'a>(
        &self,
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

#[cfg(feature = "shadowsocks")]
#[async_trait]
impl UdpFlowCapability for ShadowsocksAdapter {
    fn prepare_udp_flow<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation + 'a>,
        FlowFailure,
    > {
        self.prepare_udp_flow_impl(leaf)
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
#[async_trait]
impl TcpOutboundCapability for ShadowsocksAdapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        named_protocol_claims_runtime_leaf::<Self>(leaf)
    }

    fn outbound_leaf_runtime<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<OutboundLeafRuntime<'a>> {
        proxy_leaf_runtime(leaf, TcpPathCategory::Session)
    }

    fn prepare_tcp_connect<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpConnectOperation + 'a>,
        TcpOutboundFailure,
    > {
        self.prepare_tcp_connect_impl(leaf)
    }

    fn prepare_tcp_relay_hop<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpRelayOperation + 'a>,
        EngineError,
    > {
        self.prepare_tcp_relay_hop_impl(leaf)
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
