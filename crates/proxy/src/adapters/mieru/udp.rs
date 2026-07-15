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
    managed_stream_handler_box::<::mieru::transport::MieruManagedStreamUdpResume>(
        ManagedStreamStages::from_resume::<::mieru::transport::MieruManagedStreamUdpResume>(),
    )
}

impl MieruAdapter {
    pub(super) fn prepare_udp_flow_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let Some(leaf) = super::transport_leaf(&leaf) else {
            return Err(unreachable_udp_leaf("mieru"));
        };
        Ok(Box::new(ManagedStreamPacketUdpOperation {
            operation: PreparedManagedStreamPacketOperation::Direct {
                plan: leaf.udp_flow_plan(false).into_bridge_plan(),
            },
            needs_proxy: true,
        }))
    }

    pub(super) fn prepare_owned_udp_relay_final_hop_impl<'a>(
        &self,
        carrier: crate::transport::RelayCarrier,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let Some(leaf) = super::transport_leaf(&leaf) else {
            return Err(unreachable_udp_leaf("mieru"));
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
