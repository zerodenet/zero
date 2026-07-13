use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::unreachable_udp_leaf;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    datagram_manager::managed_datagram_socket_handler_box, ManagedDatagramFlowHandler,
};
use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor_from_build, udp_datagram_source_from_build, PacketPathCarrier,
    PacketPathCarrierDescriptor, UdpDatagramSource,
};
use zero_transport::shadowsocks_transport::ShadowsocksTransportLeaf;
use zero_transport::shadowsocks_transport::{
    ShadowsocksManagedUdpPacketPathCarrierDescriptor,
    ShadowsocksManagedUdpPacketPathDatagramSourceBuild, ShadowsocksManagedUdpPacketPathPlan,
};

impl crate::runtime::udp_flow::packet_path::UdpDatagramSourceBuild
    for ShadowsocksManagedUdpPacketPathDatagramSourceBuild
{
    fn into_parts(
        self,
    ) -> (
        String,
        String,
        u16,
        String,
        std::sync::Arc<
            dyn zero_traits::DatagramCodec<zero_core::Address, Error = zero_core::Error>,
        >,
    ) {
        ShadowsocksManagedUdpPacketPathDatagramSourceBuild::into_shared_codec_parts(self)
    }
}

impl crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptorBuild
    for ShadowsocksManagedUdpPacketPathCarrierDescriptor
{
    fn into_parts(self) -> (String, String, u16) {
        ShadowsocksManagedUdpPacketPathCarrierDescriptor::into_parts(self)
    }
}

fn packet_path_carrier_descriptor(
    plan: ShadowsocksManagedUdpPacketPathPlan<'_>,
) -> PacketPathCarrierDescriptor {
    packet_path_carrier_descriptor_from_build(plan.into_carrier_descriptor())
}

async fn build_packet_path(
    services: crate::protocol_registry::UdpRuntimeServices,
    plan: ShadowsocksManagedUdpPacketPathPlan<'_>,
) -> Result<std::sync::Arc<dyn PacketPathCarrier>, EngineError> {
    services
        .build_udp_socket_carrier(plan.server(), plan.port(), plan.carrier_codec())
        .await
}

fn packet_path_datagram_source(plan: ShadowsocksManagedUdpPacketPathPlan<'_>) -> UdpDatagramSource {
    udp_datagram_source_from_build(plan.into_datagram_source_build())
}

struct ShadowsocksPacketPathOperation<'a> {
    plan: ShadowsocksManagedUdpPacketPathPlan<'a>,
}

impl PreparedUdpPacketPathOperation for ShadowsocksPacketPathOperation<'_> {
    fn into_carrier_descriptor(self: Box<Self>) -> Option<PacketPathCarrierDescriptor> {
        Some(packet_path_carrier_descriptor(self.plan))
    }

    fn into_datagram_source(self: Box<Self>) -> Option<UdpDatagramSource> {
        Some(packet_path_datagram_source(self.plan))
    }

    fn build_carrier<'a>(
        self: Box<Self>,
        ctx: crate::protocol_registry::UdpAdapterContext<'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<std::sync::Arc<dyn PacketPathCarrier>, EngineError>,
                > + Send
                + 'a,
        >,
    >
    where
        Self: 'a,
    {
        Box::pin(async move { build_packet_path(ctx.runtime_services(), self.plan).await })
    }
}

pub(crate) fn managed_datagram_handler() -> Box<dyn ManagedDatagramFlowHandler> {
    managed_datagram_socket_handler_box::<
        zero_transport::shadowsocks_transport::ShadowsocksManagedDatagramFlowResume,
    >()
}

impl ShadowsocksAdapter {
    pub(super) fn prepare_udp_packet_path_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn PreparedUdpPacketPathOperation + 'a>> {
        let leaf = ShadowsocksTransportLeaf::from_resolved_leaf(leaf)?;
        Some(Box::new(ShadowsocksPacketPathOperation {
            plan: leaf.udp_packet_path_plan().ok()?,
        }))
    }

    pub(super) fn prepare_udp_flow_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<
        Box<dyn crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation + 'a>,
        FlowFailure,
    > {
        let Some(leaf) = ShadowsocksTransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let plan = leaf.udp_flow_plan().map_err(|error| FlowFailure {
            stage: "udp_shadowsocks_resume",
            error: EngineError::Io(std::io::Error::other(error.to_string())),
            upstream: Some((leaf.server().to_string(), leaf.port())),
        })?;
        Ok(Box::new(
            crate::runtime::udp_dispatch::operation::ManagedDatagramUdpOperation {
                plan: plan.into_start_plan(),
                needs_proxy: true,
            },
        ))
    }
}
