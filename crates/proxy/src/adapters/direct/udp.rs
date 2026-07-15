use zero_engine::ResolvedLeafOutbound;

use crate::adapters::direct::DirectAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::protocol_registry::{unreachable_udp_leaf, ClaimedUdpFlowLeaf};
use crate::runtime::udp_dispatch::operation::{DirectUdpFlowOperation, PreparedUdpFlowOperation};
use crate::runtime::udp_dispatch::FlowFailure;

struct ClaimedDirectUdpLeaf {
    tag: String,
}

impl<'a> ClaimedUdpFlowLeaf<'a> for ClaimedDirectUdpLeaf {
    fn prepare_udp_flow(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        Ok(Box::new(DirectUdpFlowOperation {
            tag: self.tag.clone(),
        }))
    }
}

impl DirectAdapter {
    pub(super) fn claim_udp_flow_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedUdpFlowLeaf<'a> + 'a>> {
        let ResolvedLeafOutbound::Direct { tag } = &leaf else {
            return None;
        };
        Some(Box::new(ClaimedDirectUdpLeaf {
            tag: (*tag).unwrap_or("direct").to_owned(),
        }))
    }

    pub(super) fn prepare_udp_flow_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let ResolvedLeafOutbound::Direct { tag } = &leaf else {
            return Err(unreachable_udp_leaf(self.name()));
        };
        Ok(Box::new(DirectUdpFlowOperation {
            tag: (*tag).unwrap_or("direct").to_owned(),
        }))
    }
}
