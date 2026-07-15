use zero_engine::ResolvedLeafOutbound;

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::ClaimedTcpOutboundLeaf;
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, SessionTcpConnectOperation,
};
use crate::transport::TcpOutboundFailure;

struct ClaimedHysteria2TcpLeaf {
    leaf: ::hysteria2::transport::Hysteria2TransportLeaf,
}

impl<'a> ClaimedTcpOutboundLeaf<'a> for ClaimedHysteria2TcpLeaf {
    fn prepare_tcp_connect(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        Ok(Box::new(SessionTcpConnectOperation {
            handshake: self.leaf.clone(),
        }))
    }
}

impl Hysteria2Adapter {
    pub(super) fn claim_tcp_outbound_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        Some(Box::new(ClaimedHysteria2TcpLeaf {
            leaf: super::transport_leaf(&leaf)?,
        }))
    }
}
