use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::ClaimedTcpOutboundLeaf;
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation, SocketTcpConnectOperation,
    SocketTcpRelayOperation,
};
use crate::transport::TcpOutboundFailure;

struct ClaimedMieruTcpLeaf {
    leaf: ::mieru::transport::MieruTransportLeaf,
}

impl<'a> ClaimedTcpOutboundLeaf<'a> for ClaimedMieruTcpLeaf {
    fn prepare_tcp_connect(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        Ok(Box::new(SocketTcpConnectOperation {
            handshake: self.leaf.clone(),
        }))
    }

    fn prepare_tcp_relay_hop(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        Ok(Box::new(SocketTcpRelayOperation {
            handshake: self.leaf.clone(),
        }))
    }
}

impl MieruAdapter {
    pub(super) fn claim_tcp_outbound_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        Some(Box::new(ClaimedMieruTcpLeaf {
            leaf: super::transport_leaf(&leaf)?,
        }))
    }
}
