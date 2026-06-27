use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::trojan::TrojanAdapter;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedStreamFlowHandler;
use crate::runtime::Proxy;

mod flow;
mod manager;

pub(crate) fn managed_stream_handler() -> Box<dyn ManagedStreamFlowHandler> {
    Box::new(manager::TrojanChainManager::new())
}

impl TrojanAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        flow::start(self, dispatch, proxy, session, leaf, payload).await
    }

    pub(super) async fn start_udp_relay_final_hop_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        flow::start_relay_final_hop(self, dispatch, proxy, session, carrier, leaf, payload).await
    }
}
