use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::unreachable_leaf;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation, SocketTcpConnectOperation,
    SocketTcpRelayOperation,
};
use crate::transport::TcpOutboundFailure;
use zero_transport::shadowsocks_transport::ShadowsocksTransportLeaf;

impl ShadowsocksAdapter {
    pub(super) fn prepare_tcp_connect_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        let Some(leaf) = ShadowsocksTransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        Ok(Box::new(SocketTcpConnectOperation { handshake: leaf }))
    }

    pub(super) fn prepare_tcp_relay_hop_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedTcpRelayOperation + 'a>, EngineError> {
        let Some(leaf) = ShadowsocksTransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        Ok(Box::new(SocketTcpRelayOperation { handshake: leaf }))
    }
}
