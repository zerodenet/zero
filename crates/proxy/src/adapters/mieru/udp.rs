use zero_engine::ResolvedLeafOutbound;

use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::ClaimedUdpFlowLeaf;
use crate::runtime::udp_dispatch::operation::{
    ManagedStreamPacketUdpOperation, PreparedManagedStreamPacketOperation, PreparedUdpFlowOperation,
};
use crate::runtime::udp_dispatch::relay::PreparedUdpRelayOperation;
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

struct ClaimedMieruUdpLeaf {
    leaf: ::mieru::transport::MieruTransportLeaf,
}

struct PreparedMieruUdpRelay {
    plan: zero_transport::managed_udp::ManagedStreamPacketBridgePlan<
        ::mieru::transport::MieruManagedStreamUdpResume,
    >,
}

impl<'a> ClaimedUdpFlowLeaf<'a> for ClaimedMieruUdpLeaf {
    fn prepare_udp_flow(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        Ok(Box::new(ManagedStreamPacketUdpOperation {
            operation: PreparedManagedStreamPacketOperation::Direct {
                plan: self.leaf.clone().udp_flow_plan(false).into_bridge_plan(),
            },
            needs_proxy: true,
        }))
    }

    fn prepare_udp_relay(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpRelayOperation<'a> + 'a>, FlowFailure> {
        Ok(Box::new(PreparedMieruUdpRelay {
            plan: self.leaf.clone().udp_flow_plan(true).into_bridge_plan(),
        }))
    }
}

impl<'a> PreparedUdpRelayOperation<'a> for PreparedMieruUdpRelay {
    fn bind_final_hop(
        self: Box<Self>,
        carrier: crate::transport::RelayCarrier,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        Ok(Box::new(ManagedStreamPacketUdpOperation {
            operation: PreparedManagedStreamPacketOperation::RelayFinalHop {
                plan: self.plan,
                carrier,
            },
            needs_proxy: false,
        }))
    }
}

impl MieruAdapter {
    pub(super) fn claim_udp_flow_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
        Some(Box::new(ClaimedMieruUdpLeaf {
            leaf: super::transport_leaf(&leaf)?,
        }))
    }
}
