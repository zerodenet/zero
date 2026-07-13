use zero_engine::ResolvedLeafOutbound;

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::unreachable_leaf;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::tcp_dispatch::operation::{
    PreparedTcpConnectOperation, SessionTcpConnectOperation,
};
use crate::transport::TcpOutboundFailure;

impl Hysteria2Adapter {
    pub(super) fn prepare_tcp_connect_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        let Some(leaf) = super::transport_leaf(leaf) else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        Ok(Box::new(SessionTcpConnectOperation { handshake: leaf }))
    }
}
