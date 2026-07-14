use zero_engine::ResolvedLeafOutbound;

use crate::adapters::direct::DirectAdapter;
use crate::protocol_registry::unreachable_leaf;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::tcp_dispatch::operation::{
    DirectTcpConnectOperation, PreparedTcpConnectOperation,
};
use crate::transport::TcpOutboundFailure;

impl DirectAdapter {
    pub(super) fn prepare_tcp_connect_impl<'a>(
        &self,
        leaf: &'a ResolvedLeafOutbound<'a>,
    ) -> Result<Box<dyn PreparedTcpConnectOperation + 'a>, TcpOutboundFailure> {
        let ResolvedLeafOutbound::Direct { tag } = leaf else {
            return Err(unreachable_leaf(self.name()));
        };
        Ok(Box::new(DirectTcpConnectOperation {
            tag: (*tag).unwrap_or("direct").to_owned(),
        }))
    }
}
