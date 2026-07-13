use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};
use zero_transport::socks5_transport::Socks5TransportLeaf;

use crate::adapters::socks5::Socks5Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::registered::UpstreamAssociationHandler;
use crate::runtime::Proxy;

mod flow;
mod packet_path;
mod upstream_association;

pub(crate) fn upstream_association_handler() -> Box<dyn UpstreamAssociationHandler> {
    flow::upstream_association_handler()
}

impl Socks5Adapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        let leaf = Socks5TransportLeaf::from_resolved_leaf(leaf)?;
        Some(packet_path::carrier_descriptor(leaf.udp_packet_path_plan()))
    }

    pub(super) async fn build_udp_packet_path_impl(
        &self,
        proxy: &Proxy,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        let Some(leaf) = Socks5TransportLeaf::from_resolved_leaf(leaf) else {
            return Err(EngineError::Io(std::io::Error::other(format!(
                "{} adapter received unsupported packet-path leaf: {leaf:?}",
                self.name()
            ))));
        };
        packet_path::build(proxy, leaf.udp_packet_path_plan()).await
    }

    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let Some(leaf) = Socks5TransportLeaf::from_resolved_leaf(leaf) else {
            return Err(FlowFailure {
                stage: "udp_unsupported_leaf",
                error: EngineError::Io(std::io::Error::other(format!(
                    "{} adapter received unsupported UDP leaf: {leaf:?}",
                    self.name()
                ))),
                upstream: None,
            });
        };
        flow::start(dispatch, proxy, session, payload, leaf.udp_flow_plan()).await
    }
}
