use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;
use zero_transport::mieru_transport::MieruTransportLeaf;

use crate::adapters::mieru::MieruAdapter;
use crate::protocol_registry::unreachable_udp_leaf;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::{
    bridge::{managed_stream_handler_box, ManagedStreamStages},
    ManagedStreamHandlerPair,
};
use crate::runtime::Proxy;

mod flow;

pub(crate) fn managed_stream_handler() -> ManagedStreamHandlerPair {
    managed_stream_handler_box::<zero_transport::mieru_transport::MieruManagedStreamUdpResume>(
        ManagedStreamStages::from_resume::<
            zero_transport::mieru_transport::MieruManagedStreamUdpResume,
        >(),
    )
}

impl MieruAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let Some(leaf) = MieruTransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_udp_leaf("mieru", leaf));
        };
        flow::start(dispatch, proxy, session, payload, leaf.udp_flow_plan(false)).await
    }

    pub(super) async fn start_udp_relay_final_hop_impl(
        &self,
        dispatch: &mut UdpDispatch,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let Some(leaf) = MieruTransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_udp_leaf("mieru", leaf));
        };
        flow::start_relay_final_hop(
            dispatch,
            session,
            carrier,
            payload,
            leaf.udp_flow_plan(true),
        )
        .await
    }
}
