use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;
use crate::runtime::Proxy;

mod flow;
mod managed;
mod packet_path;

pub(crate) fn managed_datagram_handler() -> Box<dyn ManagedDatagramFlowHandler> {
    managed::handler()
}

impl ShadowsocksAdapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        packet_path::carrier_descriptor(leaf)
    }

    pub(super) async fn build_udp_packet_path_impl(
        &self,
        proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        packet_path::build(self, proxy, leaf).await
    }

    pub(super) fn udp_datagram_source_impl<'a>(
        &self,
        leaf: &ResolvedLeafOutbound<'a>,
    ) -> Option<crate::runtime::udp_flow::packet_path::UdpDatagramSource<'a>> {
        packet_path::datagram_source(leaf)
    }

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
}
