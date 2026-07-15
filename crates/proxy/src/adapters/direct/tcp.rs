use zero_engine::ResolvedLeafOutbound;

use crate::adapters::direct::DirectAdapter;
use crate::protocol_registry::{direct_leaf_runtime, ClaimedTcpOutboundLeaf, OutboundLeafRuntime};
use crate::runtime::tcp_dispatch::operation::{
    DirectTcpConnectOperation, PreparedTcpConnectOperation,
};
use crate::transport::TcpOutboundFailure;

struct ClaimedDirectTcpLeaf {
    runtime: OutboundLeafRuntime,
    tag: String,
}

impl<'a> ClaimedTcpOutboundLeaf<'a> for ClaimedDirectTcpLeaf {
    fn runtime(&self) -> OutboundLeafRuntime {
        self.runtime.clone()
    }

    fn prepare_tcp_connect(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        Ok(Box::new(DirectTcpConnectOperation {
            tag: self.tag.clone(),
        }))
    }
}

impl DirectAdapter {
    pub(super) fn claim_tcp_outbound_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        let runtime = direct_leaf_runtime(&leaf)?;
        let ResolvedLeafOutbound::Direct { tag } = &leaf else {
            return None;
        };
        Some(Box::new(ClaimedDirectTcpLeaf {
            runtime,
            tag: (*tag).unwrap_or("direct").to_owned(),
        }))
    }
}
