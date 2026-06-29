use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::vless::VlessAdapter;
use crate::runtime::udp_dispatch::{FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;

mod flow;
mod managed;

impl VlessAdapter {
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

    pub(super) fn udp_relay_needs_two_streams_impl(&self, leaf: &ResolvedLeafOutbound<'_>) -> bool {
        let _ = self;
        matches!(
            leaf,
            ResolvedLeafOutbound::Vless {
                split_http: Some(cfg),
                ..
            } if crate::transport::vless_udp_relay_needs_two_streams(Some(cfg))
        )
    }

    pub(super) async fn start_udp_relay_two_stream_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        chain: Vec<ResolvedLeafOutbound<'_>>,
        payload: &[u8],
    ) -> Result<FlowStartResult, crate::runtime::udp_dispatch::FlowFailure> {
        flow::start_relay_two_stream(self, dispatch, proxy, session, chain, payload).await
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
