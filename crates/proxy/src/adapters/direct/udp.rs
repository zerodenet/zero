use crate::adapters::direct::DirectAdapter;
use crate::protocol_registry::ClaimedUdpFlowLeaf;
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
        tag: String,
    ) -> Box<dyn ClaimedUdpFlowLeaf<'a> + 'a> {
        Box::new(ClaimedDirectUdpLeaf { tag })
    }
}
