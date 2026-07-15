use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::direct::DirectAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::protocol_registry::{unreachable_leaf, ClaimedTcpOutboundLeaf};
use crate::runtime::tcp_dispatch::operation::{
    DirectTcpConnectOperation, PreparedTcpConnectOperation, PreparedTcpRelayOperation,
};
use crate::transport::TcpOutboundFailure;

struct ClaimedDirectTcpLeaf {
    tag: String,
}

impl<'a> ClaimedTcpOutboundLeaf<'a> for ClaimedDirectTcpLeaf {
    fn prepare_tcp_connect(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        Ok(Box::new(DirectTcpConnectOperation {
            tag: self.tag.clone(),
        }))
    }

    fn prepare_tcp_relay_hop(
        &self,
        _source_dir: Option<&std::path::Path>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "this adapter does not support relay hop",
        )))
    }
}

impl DirectAdapter {
    pub(super) fn claim_tcp_outbound_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        let ResolvedLeafOutbound::Direct { tag } = &leaf else {
            return None;
        };
        Some(Box::new(ClaimedDirectTcpLeaf {
            tag: (*tag).unwrap_or("direct").to_owned(),
        }))
    }

    pub(super) fn prepare_tcp_connect_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Direct { tag } = &leaf else {
            return Err(unreachable_leaf(self.name()));
        };
        Ok(Box::new(DirectTcpConnectOperation {
            tag: (*tag).unwrap_or("direct").to_owned(),
        }))
    }
}
