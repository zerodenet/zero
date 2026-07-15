use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::protocol_registry::{unreachable_leaf, ClaimedTcpOutboundLeaf};
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation, SocketTcpConnectOperation,
    SocketTcpRelayOperation,
};
use crate::transport::TcpOutboundFailure;

struct ClaimedSocks5TcpLeaf {
    leaf: ::socks5::transport::Socks5TransportLeaf,
}

impl<'a> ClaimedTcpOutboundLeaf<'a> for ClaimedSocks5TcpLeaf {
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

impl Socks5Adapter {
    pub(super) fn claim_tcp_outbound_leaf_impl<'a>(
        &self,
        leaf: ResolvedLeafOutbound<'a>,
    ) -> Option<Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a>> {
        Some(Box::new(ClaimedSocks5TcpLeaf {
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
