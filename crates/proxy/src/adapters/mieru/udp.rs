use zero_engine::ResolvedLeafOutbound;

use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::unreachable_udp_leaf;
use crate::runtime::udp_dispatch::operation::{
    ManagedStreamPacketUdpOperation, PreparedManagedStreamPacketOperation, PreparedUdpFlowOperation,
};
use crate::runtime::udp_dispatch::FlowFailure;
use crate::runtime::udp_flow::managed::{
    bridge::{managed_stream_handler_box, ManagedStreamStages},
    ManagedStreamHandlerPair,
};

pub(crate) fn managed_stream_handler() -> ManagedStreamHandlerPair {
    managed_stream_handler_box::<zero_transport::mieru_transport::MieruManagedStreamUdpResume>(
        ManagedStreamStages::from_resume::<
            zero_transport::mieru_transport::MieruManagedStreamUdpResume,
        >(),
    )
}

impl MieruAdapter {
    pub(super) fn prepare_udp_flow_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let Some(leaf) = super::transport_leaf(leaf) else {
            return Err(unreachable_udp_leaf("mieru", leaf));
        };
        Ok(Box::new(ManagedStreamPacketUdpOperation {
            operation: PreparedManagedStreamPacketOperation::Direct {
                plan: leaf.udp_flow_plan(false).into_bridge_plan(),
            },
            needs_proxy: true,
        }))
    }

    pub(super) fn prepare_udp_relay_final_hop_impl<'a>(
        &self,
        carrier: crate::transport::RelayCarrier,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let Some(leaf) = super::transport_leaf(leaf) else {
            return Err(unreachable_udp_leaf("mieru", leaf));
        };
        Ok(Box::new(ManagedStreamPacketUdpOperation {
            operation: PreparedManagedStreamPacketOperation::RelayFinalHop {
                plan: leaf.udp_flow_plan(true).into_bridge_plan(),
                carrier,
            },
            needs_proxy: false,
        }))
    }
}
