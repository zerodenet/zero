use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::protocol_registry::{unreachable_leaf, ClaimedTcpOutboundLeaf};
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation, SocketTcpConnectOperation,
    SocketTcpRelayOperation,
};
use crate::transport::TcpOutboundFailure;

struct ClaimedShadowsocksTcpLeaf {
    leaf: ::shadowsocks::transport::ShadowsocksTransportLeaf,
}

impl<'a> ClaimedTcpOutboundLeaf<'a> for ClaimedShadowsocksTcpLeaf {
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

impl ShadowsocksAdapter {
    pub(super) fn claim_tcp_outbound_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        Some(Box::new(ClaimedShadowsocksTcpLeaf {
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
        Ok(Box::new(SocketTcpConnectOperation { handshake: leaf }))
    }

    pub(super) fn prepare_tcp_relay_hop_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        let Some(leaf) = super::transport_leaf(&leaf) else {
            return Err(unreachable_leaf(self.name()).error);
        };
        Ok(Box::new(SocketTcpRelayOperation { handshake: leaf }))
    }
}
