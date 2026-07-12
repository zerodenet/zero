use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::protocol_registry::{unreachable_leaf, unreachable_udp_leaf};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;
use zero_transport::hysteria2_quic::Hysteria2TransportLeaf;

mod flow;
mod managed;
mod packet_path;

pub(crate) fn managed_datagram_handler() -> Box<dyn ManagedDatagramFlowHandler> {
    managed::handler()
}

impl Hysteria2Adapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        let leaf = Hysteria2TransportLeaf::from_resolved_leaf(leaf)?;
        Some(packet_path::carrier_descriptor(leaf.udp_packet_path_plan()))
    }

    pub(super) async fn build_udp_packet_path_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        let Some(leaf) = Hysteria2TransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        packet_path::build(leaf.udp_packet_path_plan()).await
    }

    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let Some(leaf) = Hysteria2TransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        flow::start(dispatch, session, payload, leaf.udp_flow_plan()).await
    }
}
