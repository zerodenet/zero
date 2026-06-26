use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::hysteria2_flow::Hysteria2DatagramSend;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};

impl Hysteria2Adapter {
    #[cfg(feature = "shadowsocks")]
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::PacketPathCarrierDescriptor> {
        let _ = self;
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
        Some(
            crate::protocol_runtime::udp::packet_path_snapshot::packet_path_carrier_descriptor(
                hysteria2::udp_cache_key(tag, server, *port, password, *client_fingerprint),
                server,
                *port,
            ),
        )
    }

    #[cfg(feature = "shadowsocks")]
    pub(super) async fn build_udp_packet_path_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Result<Arc<dyn crate::protocol_runtime::udp::PacketPathCarrier>, EngineError> {
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
        crate::protocol_runtime::udp::packet_path_chain::carriers::build_hysteria2_packet_path(
            server,
            *port,
            password,
            *client_fingerprint,
        )
        .await
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
        dispatch
            .start_hysteria2_datagram_flow(Hysteria2DatagramSend {
                tag,
                session,
                server,
                port: *port,
                password,
                client_fingerprint: *client_fingerprint,
                payload,
            })
            .await
    }
}
