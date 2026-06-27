use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::socks5::Socks5Adapter;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::registered::UpstreamAssociationHandler;
use crate::runtime::Proxy;

mod active;
mod establish;
mod flow;
mod model;
mod packet_path;
mod runtime;
mod send;

pub(crate) fn upstream_association_handler() -> Box<dyn UpstreamAssociationHandler> {
    Box::<runtime::Socks5UdpRuntime>::default()
}

impl Socks5Adapter {
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
