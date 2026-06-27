use zero_engine::EngineError;

use super::super::ProtocolInventory;
use crate::protocol_registry::{UdpAdapterContext, UdpFlowCapability};
use crate::runtime::Proxy;

impl ProtocolInventory {
    /// Whether the UDP relay final hop needs the VLESS two-stream path.
    pub(crate) fn udp_relay_needs_two_streams(
        &self,
        leaf: &zero_engine::ResolvedLeafOutbound<'_>,
    ) -> Result<bool, EngineError> {
        let adapter = self.registry.find_outbound_leaf(leaf)?;
        Ok(UdpFlowCapability::udp_relay_needs_two_streams(
            adapter.as_ref(),
            leaf,
        ))
    }

    /// Start a two-stream UDP relay path through the final hop adapter.
    pub(crate) async fn start_udp_relay_two_stream(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        proxy: &Proxy,
        session: &zero_core::Session,
        chain: Vec<zero_engine::ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<
        crate::runtime::udp_dispatch::FlowStartResult,
        crate::runtime::udp_dispatch::FlowFailure,
    > {
        let final_hop = chain.last().expect("relay chain has at least 2 hops");
        let adapter = self
            .registry
            .find_outbound_leaf(final_hop)
            .map_err(|error| crate::runtime::udp_dispatch::FlowFailure {
                stage: "find_outbound_leaf",
                error,
                upstream: None,
            })?;
        UdpFlowCapability::start_udp_relay_two_stream(
            adapter.as_ref(),
            dispatch,
            UdpAdapterContext::new(proxy),
            session,
            chain,
            payload,
        )
        .await
    }

    /// Start a single-stream UDP relay final hop through the final hop adapter.
    pub(crate) async fn start_udp_relay_final_hop(
        &self,
        dispatch: &mut crate::runtime::udp_dispatch::UdpDispatch,
        proxy: &Proxy,
        session: &zero_core::Session,
        carrier: crate::transport::RelayCarrier,
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
        UdpFlowCapability::start_udp_relay_final_hop(
            adapter.as_ref(),
            dispatch,
            UdpAdapterContext::new(proxy),
            session,
            carrier,
            leaf,
            payload,
        )
        .await
    }
}
