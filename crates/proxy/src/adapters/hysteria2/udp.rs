use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_registry::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::managed::ManagedDatagramFlowHandler;

mod connector;
mod flow;
mod managed;
mod packet_path;

pub(crate) struct Hysteria2UdpFlowStart<'a> {
    pub(crate) tag: &'a str,
    pub(crate) server: &'a str,
    pub(crate) port: u16,
    pub(crate) resume: hysteria2::udp::Hysteria2UdpFlowResume,
}

pub(crate) fn managed_datagram_handler() -> Box<dyn ManagedDatagramFlowHandler> {
    managed::handler()
}

impl Hysteria2Adapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
        let ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
            ..
        } = leaf
        else {
            return None;
        };
        let descriptor = hysteria2::udp::udp_packet_path_carrier_descriptor_from_config(
            tag,
            server,
            *port,
            password,
            *client_fingerprint,
        );
        Some(packet_path::carrier_descriptor(descriptor))
    }

    pub(super) async fn build_udp_packet_path_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<
        std::sync::Arc<dyn crate::runtime::udp_flow::packet_path::PacketPathCarrier>,
        EngineError,
    > {
        let ResolvedLeafOutbound::Hysteria2 {
            server,
            port,
            password,
            client_fingerprint,
            ..
        } = leaf
        else {
            return Err(unreachable_leaf(self.name(), leaf).error);
        };
        let carrier = hysteria2::udp::udp_packet_path_carrier_build_from_config(
            "",
            server,
            *port,
            password,
            *client_fingerprint,
        );
        packet_path::build(carrier).await
    }

    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Hysteria2 {
            tag,
            server,
            port,
            password,
            client_fingerprint,
            ..
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        let resume = hysteria2::udp::udp_flow_resume_from_config(
            tag,
            server,
            *port,
            password,
            *client_fingerprint,
        );
        let request = Hysteria2UdpFlowStart {
            tag,
            server,
            port: *port,
            resume,
        };
        flow::start(dispatch, session, request, payload).await
    }
}
