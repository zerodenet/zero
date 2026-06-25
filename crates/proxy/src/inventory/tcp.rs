use zero_engine::EngineError;

use super::ProtocolInventory;
use crate::protocol_adapter::{OutboundAdapterContext, TcpOutboundCapability};
use crate::runtime::Proxy;
use crate::transport::{EstablishedTcpOutbound, TcpOutboundFailure, TcpRelayStream};

impl ProtocolInventory {
    /// Establish a TCP outbound through the adapter that owns `leaf`.
    pub(crate) async fn connect_tcp_leaf(
        &self,
        proxy: &Proxy,
        session: &zero_core::Session,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Result<EstablishedTcpOutbound, TcpOutboundFailure> {
        let adapter =
            self.registry
                .find_outbound_leaf(leaf)
                .map_err(|error| TcpOutboundFailure {
                    stage: "find_outbound_leaf",
                    error,
                    upstream_endpoint: None,
                })?;
        TcpOutboundCapability::connect_tcp(
            adapter.as_ref(),
            OutboundAdapterContext::new(proxy),
            session,
            leaf,
        )
        .await
    }

    /// Apply one relay-chain TCP hop through the adapter that owns `leaf`.
    pub(crate) async fn apply_tcp_relay_hop(
        &self,
        proxy: &Proxy,
        stream: TcpRelayStream,
        session: &zero_core::Session,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Result<TcpRelayStream, EngineError> {
        let adapter = self.registry.find_outbound_leaf(leaf)?;
        TcpOutboundCapability::apply_relay_hop(
            adapter.as_ref(),
            OutboundAdapterContext::new(proxy),
            stream,
            session,
            leaf,
        )
        .await
    }
}
