use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::direct::DirectAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
use crate::runtime::Proxy;

impl DirectAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Direct { tag } = leaf else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let target_addr = proxy
            .protocols
            .direct_connector()
            .resolve_target_addr(session, proxy.resolver.as_ref())
            .await
            .map_err(|error| FlowFailure {
                stage: "resolve_udp_target",
                error: error.into(),
                upstream: None,
            })?;
        let sent = dispatch
            .send_direct_packet(target_addr, payload)
            .await
            .map_err(|error| FlowFailure {
                stage: "udp_direct_send",
                error,
                upstream: None,
            })?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Direct {
                tag: (*tag).unwrap_or("direct").to_string(),
                target_addr,
            }),
            tx_bytes: sent as u64,
        })
    }
}
