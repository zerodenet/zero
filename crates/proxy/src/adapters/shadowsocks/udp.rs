use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::shadowsocks::ShadowsocksAdapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::protocol_registry::{unreachable_leaf, unreachable_udp_leaf};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;
use crate::runtime::Proxy;
use zero_transport::shadowsocks_transport::ShadowsocksTransportLeaf;

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
        let leaf = ShadowsocksTransportLeaf::from_resolved_leaf(leaf)?;
        let plan = leaf.udp_packet_path_plan().ok()?;
        Some(packet_path::carrier_descriptor(plan))
    }

    pub(super) async fn build_udp_packet_path_impl(
        &self,
        proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        let Some(leaf) = ShadowsocksTransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let plan = leaf
            .udp_packet_path_plan()
            .map_err(|error| EngineError::Io(std::io::Error::other(error.to_string())))?;
        packet_path::build(proxy, plan).await
    }

    pub(super) fn udp_datagram_source_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::UdpDatagramSource> {
        let leaf = ShadowsocksTransportLeaf::from_resolved_leaf(leaf)?;
        let plan = leaf.udp_packet_path_plan().ok()?;
        Some(packet_path::datagram_source(plan))
    }

    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let Some(leaf) = ShadowsocksTransportLeaf::from_resolved_leaf(leaf) else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let plan = leaf.udp_flow_plan().map_err(|error| FlowFailure {
            stage: "udp_shadowsocks_resume",
            error: EngineError::Io(std::io::Error::other(error.to_string())),
            upstream: Some((leaf.server().to_string(), leaf.port())),
        })?;
        flow::start(dispatch, proxy, session, payload, plan).await
    }
}
