use zero_engine::ResolvedLeafOutbound;

use crate::adapters::direct::DirectAdapter;
use crate::protocol_registry::unreachable_udp_leaf;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::operation::{DirectUdpFlowOperation, PreparedUdpFlowOperation};
use crate::runtime::udp_dispatch::FlowFailure;

impl DirectAdapter {
    pub(super) fn prepare_udp_flow_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedUdpFlowOperation + 'a>, FlowFailure> {
        let ResolvedLeafOutbound::Direct { tag } = leaf else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        Ok(Box::new(DirectUdpFlowOperation {
            tag: (*tag).unwrap_or("direct").to_owned(),
        }))
    }
}
