use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::vmess::VmessAdapter;
use crate::runtime::udp_dispatch::{FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedStreamFlowHandler;
use crate::runtime::Proxy;

mod flow;
mod managed;

pub(crate) fn managed_stream_handler() -> Box<dyn ManagedStreamFlowHandler> {
    managed::handler()
}

impl VmessAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, crate::runtime::udp_dispatch::FlowFailure> {
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
    ) -> Result<FlowStartResult, crate::runtime::udp_dispatch::FlowFailure> {
        flow::start_relay_final_hop(self, dispatch, proxy, session, carrier, leaf, payload).await
    }
}
