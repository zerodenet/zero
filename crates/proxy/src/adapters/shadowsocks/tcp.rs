use zero_engine::EngineError;

use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ClaimedTcpOutboundLeaf;
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
        leaf: ::shadowsocks::transport::ShadowsocksTransportLeaf,
    ) -> Box<dyn ClaimedTcpOutboundLeaf<'a> + 'a> {
        Box::new(ClaimedShadowsocksTcpLeaf { leaf })
    }
}
