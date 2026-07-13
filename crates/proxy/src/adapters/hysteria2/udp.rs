use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::unreachable_udp_leaf;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    datagram_manager::managed_datagram_handler_box, ManagedDatagramFlowHandler,
};
use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor_from_build, DatagramCodec, PacketPathCarrier,
    PacketPathCarrierDescriptor, PacketPathCarrierDescriptorBuild,
};
use zero_transport::hysteria2_quic::Hysteria2TransportLeaf;
use zero_transport::hysteria2_quic::{
    Hysteria2ManagedUdpPacketPathCarrierDescriptor, Hysteria2ManagedUdpPacketPathPlan,
};

impl PacketPathCarrierDescriptorBuild for Hysteria2ManagedUdpPacketPathCarrierDescriptor {
    fn into_parts(self) -> (String, String, u16) {
        Hysteria2ManagedUdpPacketPathCarrierDescriptor::into_parts(self)
    }
}

fn packet_path_carrier_descriptor(
    plan: Hysteria2ManagedUdpPacketPathPlan,
) -> PacketPathCarrierDescriptor {
    packet_path_carrier_descriptor_from_build(plan.into_carrier_descriptor())
}

async fn build_packet_path(
    plan: Hysteria2ManagedUdpPacketPathPlan,
) -> Result<std::sync::Arc<dyn PacketPathCarrier>, EngineError> {
    let (conn, codec): (
        quinn::Connection,
        std::sync::Arc<dyn DatagramCodec<zero_core::Address, Error = zero_core::Error>>,
    ) = zero_transport::hysteria2_quic::open_hysteria2_udp_packet_path_build(
        plan.into_carrier_build(),
    )
    .await?;
    crate::runtime::udp_flow::packet_path_chain::carriers::quic_datagram_carrier::build(
        std::sync::Arc::new(conn),
        codec,
    )
    .await
}

struct Hysteria2PacketPathOperation {
    plan: Hysteria2ManagedUdpPacketPathPlan,
}

impl PreparedUdpPacketPathOperation for Hysteria2PacketPathOperation {
    fn into_carrier_descriptor(self: Box<Self>) -> Option<PacketPathCarrierDescriptor> {
        Some(packet_path_carrier_descriptor(self.plan))
    }

    fn build_carrier<'a>(
        self: Box<Self>,
        _ctx: crate::protocol_registry::UdpAdapterContext<'a>,
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
        Box::pin(async move { build_packet_path(self.plan).await })
    }
}

pub(crate) fn managed_datagram_handler() -> Box<dyn ManagedDatagramFlowHandler> {
    managed_datagram_handler_box::<zero_transport::hysteria2_quic::Hysteria2ManagedDatagramFlowResume>(
    )
}

impl Hysteria2Adapter {
    pub(super) fn prepare_udp_packet_path_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn PreparedUdpPacketPathOperation + 'a>> {
        let leaf = Hysteria2TransportLeaf::from_resolved_leaf(leaf)?;
        Some(Box::new(Hysteria2PacketPathOperation {
            plan: leaf.udp_packet_path_plan(),
        }))
    }

    pub(super) fn prepare_udp_flow_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<
        Box<dyn crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation + 'a>,
        FlowFailure,
    > {
        let Some(leaf) = Hysteria2TransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        Ok(Box::new(
            crate::runtime::udp_dispatch::operation::ManagedDatagramUdpOperation {
                plan: leaf.udp_flow_plan().into_start_plan(),
                needs_proxy: false,
            },
        ))
    }
}
