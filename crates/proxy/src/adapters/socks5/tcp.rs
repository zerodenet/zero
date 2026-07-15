use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::unreachable_leaf;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, PreparedTcpRelayOperation, SocketTcpConnectOperation,
    SocketTcpRelayOperation,
};
use crate::transport::TcpOutboundFailure;

impl Socks5Adapter {
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
