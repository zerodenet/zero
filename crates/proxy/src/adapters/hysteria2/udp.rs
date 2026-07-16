use zero_engine::EngineError;

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::{ClaimedUdpFlowLeaf, ClaimedUdpPacketPathLeaf};
use crate::runtime::udp_dispatch::operation::PreparedUdpFlowOperation;
use crate::runtime::udp_dispatch::packet_path_operation::PreparedUdpPacketPathOperation;
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    datagram_manager::managed_datagram_handler_box, ManagedDatagramFlowHandler,
};
use crate::runtime::udp_flow::packet_path::{
    packet_path_carrier_descriptor_from_build, DatagramCodec, PacketPathCarrier,
    PacketPathCarrierDescriptor, PacketPathCarrierDescriptorBuild,
};
use ::hysteria2::transport::{
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
    ) = ::hysteria2::transport::open_hysteria2_udp_packet_path_build(plan.into_carrier_build())
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

struct ClaimedHysteria2PacketPathLeaf {
    plan: Hysteria2ManagedUdpPacketPathPlan,
}

struct ClaimedHysteria2UdpLeaf {
    leaf: ::hysteria2::transport::Hysteria2TransportLeaf,
}

impl<'a> ClaimedUdpFlowLeaf<'a> for ClaimedHysteria2UdpLeaf {
    fn prepare_udp_flow(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        Ok(Box::new(
            crate::runtime::udp_dispatch::operation::ManagedDatagramUdpOperation {
                plan: self.leaf.clone().udp_flow_plan().into_start_plan(),
                needs_proxy: false,
            },
        ))
    }
}

impl<'a> ClaimedUdpPacketPathLeaf<'a> for ClaimedHysteria2PacketPathLeaf {
    fn prepare_udp_packet_path(&self) -> Option<Box<dyn PreparedUdpPacketPathOperation + 'a>> {
        Some(Box::new(Hysteria2PacketPathOperation {
            plan: self.plan.clone(),
        }))
    }
}

impl PreparedUdpPacketPathOperation for Hysteria2PacketPathOperation {
    fn carrier_descriptor(&self) -> Option<PacketPathCarrierDescriptor> {
        Some(packet_path_carrier_descriptor(self.plan.clone()))
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
    managed_datagram_handler_box::<::hysteria2::transport::Hysteria2ManagedDatagramFlowResume>()
}

impl Hysteria2Adapter {
    pub(super) fn claim_udp_flow_leaf_impl<'a>(
        &self,
        leaf: ::hysteria2::transport::Hysteria2TransportLeaf,
    ) -> Box<dyn ClaimedUdpFlowLeaf<'a> + 'a> {
        Box::new(ClaimedHysteria2UdpLeaf { leaf })
    }

    pub(super) fn claim_udp_packet_path_leaf_impl<'a>(
        &self,
        leaf: ::hysteria2::transport::Hysteria2TransportLeaf,
    ) -> Option<Box<dyn ClaimedUdpPacketPathLeaf<'a> + 'a>> {
        Some(Box::new(ClaimedHysteria2PacketPathLeaf {
            plan: leaf.udp_packet_path_plan(),
        }))
    }
}
