use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::protocol_runtime::udp::{
    ManagedDatagramFlowHandler, ManagedUdpFlowKind, ProtocolUdpFlowResume,
};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_dispatch::{ManagedProtocolUdpSend, ManagedUdpOutboundKind};

pub(crate) fn managed_datagram_handler() -> Box<dyn ManagedDatagramFlowHandler> {
    Box::new(crate::protocol_runtime::udp::h2_manager::H2ChainManager::new())
}

impl Hysteria2Adapter {
    pub(super) fn udp_packet_path_carrier_descriptor_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::runtime::udp_flow::packet_path::PacketPathCarrierDescriptor> {
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
        let packet_path = hysteria2::Hysteria2UdpPacketPathConfig {
            tag,
            server,
            port: *port,
            password,
            client_fingerprint: *client_fingerprint,
        };
        Some(
            crate::runtime::udp_flow::packet_path::packet_path_carrier_descriptor(
                packet_path.cache_key(),
                server,
                *port,
            ),
        )
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
        let packet_path = hysteria2::Hysteria2UdpPacketPathConfig {
            tag: "",
            server,
            port: *port,
            password,
            client_fingerprint: *client_fingerprint,
        };
        let codec = std::sync::Arc::new(packet_path.codec());
        let conn = std::sync::Arc::new(
            crate::outbound::hysteria2::Hysteria2Connector::new(server, *port, password)
                .with_fingerprint(*client_fingerprint)
                .connect_raw()
                .await?,
        );
        crate::runtime::udp_flow::packet_path_chain::carriers::quic_datagram_carrier::build(
            conn, codec,
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
            .start_tracked_managed_protocol_udp(ManagedProtocolUdpSend {
                proxy: None,
                tag,
                session,
                carrier: None,
                tls_server_name: None,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::new(hysteria2::Hysteria2UdpFlowResume::new(
                    password,
                    *client_fingerprint,
                )),
                payload,
                kind: ManagedUdpFlowKind::Datagram,
                outbound: ManagedUdpOutboundKind::Datagram,
            })
            .await
    }
}
