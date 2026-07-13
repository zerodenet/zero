use zero_engine::EngineError;

use super::ProtocolInventory;
use crate::protocol_registry::{OutboundAdapterContext, TcpOutboundCapability};
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
        let operation = TcpOutboundCapability::prepare_tcp_connect(adapter.as_ref(), leaf)?;
        operation
            .execute(OutboundAdapterContext::new(proxy), session)
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
        let operation = TcpOutboundCapability::prepare_tcp_relay_hop(adapter.as_ref(), leaf)?;
        operation
            .execute(OutboundAdapterContext::new(proxy), stream, session)
            .await
    }
}
