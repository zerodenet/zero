use async_trait::async_trait;

use ::hysteria2::transport::Hysteria2TransportLeaf;
use ::hysteria2::transport::OwnedHysteria2InboundBindPlan;
use zero_config::{InboundConfig, InboundProtocolConfig, OutboundProtocolConfig};
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_traits::{ProtocolCapabilityDescriptor, ProtocolMetadata};

use crate::adapters::identity::{
    named_protocol_claims_runtime_leaf, named_protocol_supports_inbound,
    named_protocol_supports_outbound, NamedProtocolAdapter,
};
use crate::protocol_registry::{
    bind_transport_inbound, proxy_leaf_runtime, BoundInbound, ClaimedTcpOutboundLeaf,
    ClaimedUdpFlowLeaf, ClaimedUdpPacketPathLeaf, InboundListenerCapability,
    ManagedUdpHandlerProvider, OutboundLeafRuntime, ProtocolSupportCapability,
    TcpOutboundCapability, UdpFlowCapability, UdpPacketPathCapability,
};
use crate::runtime::path::TcpPathCategory;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;
use crate::transport::TcpOutboundFailure;

#[cfg(feature = "hysteria2")]
mod inbound;
#[cfg(feature = "hysteria2")]
mod tcp;
#[cfg(feature = "hysteria2")]
pub(crate) mod udp;

#[cfg(feature = "hysteria2")]
#[derive(Debug)]
pub(crate) struct Hysteria2Adapter;

fn transport_leaf(leaf: &ResolvedLeafOutbound<'_>) -> Option<Hysteria2TransportLeaf> {
    let ResolvedLeafOutbound::Hysteria2 {
        tag,
        server,
        port,
        password,
        client_fingerprint,
        ..
    } = leaf
    else {
        return None;
    };
    Some(Hysteria2TransportLeaf::new(
        *tag,
        *server,
        *port,
        *password,
        client_fingerprint
            .as_ref()
            .map(|client_fingerprint| (*client_fingerprint).to_owned()),
    ))
}

#[cfg(feature = "hysteria2")]
impl NamedProtocolAdapter for Hysteria2Adapter {
    const PROTOCOL_NAME: &'static str = "hysteria2";
    const FEATURE_NAME: &'static str = "hysteria2";
}

#[cfg(feature = "hysteria2")]
impl UdpPacketPathCapability for Hysteria2Adapter {
    fn claim_udp_packet_path_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>> {
        self.claim_udp_packet_path_leaf_impl(leaf)
    }

    fn prepare_udp_packet_path<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<
        Box<
            dyn crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation
                + 'a,
        >,
    > {
        self.prepare_udp_packet_path_impl(leaf)
    }
}

#[cfg(feature = "hysteria2")]
#[async_trait]
impl UdpFlowCapability for Hysteria2Adapter {
    fn claim_udp_flow_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
        self.claim_udp_flow_leaf_impl(leaf)
    }

    fn prepare_udp_flow<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation + 'a>,
        FlowFailure,
    > {
        self.prepare_udp_flow_impl(leaf)
    }
}

#[cfg(feature = "hysteria2")]
impl ManagedUdpHandlerProvider for Hysteria2Adapter {
    fn managed_datagram_udp_handler(&self) -> Option<Box<dyn ManagedDatagramFlowHandler>> {
        Some(udp::managed_datagram_handler())
    }
}

#[cfg(feature = "hysteria2")]
#[async_trait]
impl InboundListenerCapability for Hysteria2Adapter {
    async fn bind_inbound(
        &self,
        inbound: &InboundConfig,
        source_dir: Option<&std::path::Path>,
    ) -> Result<BoundInbound, EngineError> {
        let InboundProtocolConfig::Hysteria2 {
            cert_path,
            key_path,
            ..
        } = &inbound.protocol
        else {
            return Err(EngineError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "hysteria2 inbound bind received non-hysteria2 inbound config",
            )));
        };
        let plan = OwnedHysteria2InboundBindPlan::from_paths(
            source_dir,
            cert_path.as_deref(),
            key_path.as_deref(),
        );
        bind_transport_inbound(inbound, plan).await
    }

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

#[cfg(feature = "hysteria2")]
#[async_trait]
impl TcpOutboundCapability for Hysteria2Adapter {
    fn claims_outbound_leaf(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        named_protocol_claims_runtime_leaf::<Self>(leaf)
    }

    fn claim_tcp_outbound_leaf<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        self.claim_tcp_outbound_leaf_impl(leaf)
    }

    fn outbound_leaf_runtime(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<OutboundLeafRuntime> {
        proxy_leaf_runtime(leaf, TcpPathCategory::TransportSession)
    }

    fn prepare_tcp_connect<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<
        Box<dyn crate::runtime::tcp_dispatch::operation::PreparedTcpConnectOperation + 'a>,
        TcpOutboundFailure,
    > {
        self.prepare_tcp_connect_impl(leaf)
    }
}

#[cfg(feature = "hysteria2")]
impl ProtocolSupportCapability for Hysteria2Adapter {
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

#[cfg(feature = "hysteria2")]
impl ProtocolMetadata for Hysteria2Adapter {
    fn descriptor(&self) -> ProtocolCapabilityDescriptor {
        ::hysteria2::Hysteria2Protocol.descriptor()
    }
}
