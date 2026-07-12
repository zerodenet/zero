use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::unreachable_leaf;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure};
use zero_transport::hysteria2_quic::Hysteria2TransportLeaf;

impl Hysteria2Adapter {
    pub(super) async fn connect_tcp_impl(
        &self,
        _proxy: &crate::runtime::Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let Some(leaf) = Hysteria2TransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_leaf(self.name(), leaf));
        };
        match leaf.open_tcp_stream(session).await {
            Ok(upstream) => Ok(EstablishedTcpOutbound::proxied(
                leaf.tag().to_owned(),
                leaf.server().to_owned(),
                leaf.port(),
                upstream,
            )),
            Err(error) => Err(TcpOutboundFailure {
                stage: "connect_upstream_hysteria2",
                error,
                upstream_endpoint: Some((leaf.server().to_string(), leaf.port())),
            }),
        }
    }
}
