use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::protocol_registry::{unreachable_leaf, ClaimedTcpOutboundLeaf};
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation, SessionTcpConnectOperation,
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

impl Hysteria2Adapter {
    pub(super) fn claim_tcp_outbound_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        Some(Box::new(ClaimedHysteria2TcpLeaf {
            leaf: super::transport_leaf(&leaf)?,
        }))
    }

    pub(super) fn prepare_tcp_connect_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        let Some(leaf) = super::transport_leaf(&leaf) else {
            return Err(unreachable_leaf(self.name()));
        };
        Ok(Box::new(SessionTcpConnectOperation { handshake: leaf }))
    }
}
