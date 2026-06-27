use super::super::ProtocolInventory;
use crate::protocol_registry::{UdpAdapterContext, UdpFlowCapability};
use crate::runtime::Proxy;

impl ProtocolInventory {
    /// Start a single-hop UDP flow through the adapter that owns `leaf`.
    pub(crate) async fn start_udp_leaf_flow(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        proxy: &Proxy,
        session: &zero_core::Session,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        let adapter = self.registry.find_outbound_leaf(leaf).map_err(|error| {
            crate::runtime::udp_dispatch::FlowFailure {
                stage: "find_outbound_leaf",
                error,
                upstream: None,
            }
        })?;
        UdpFlowCapability::start_udp_flow(
            adapter.as_ref(),
            dispatch,
            UdpAdapterContext::new(proxy),
            session,
            leaf,
            payload,
        )
        .await
    }
}
