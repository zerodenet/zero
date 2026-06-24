use std::sync::Arc;

use zero_core::Session;
use zero_engine::{EngineError, ResolvedLeafOutbound};

use crate::adapters::common::{unreachable_leaf, unreachable_udp_leaf};
use crate::adapters::hysteria2::Hysteria2Adapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;

impl Hysteria2Adapter {
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
        Some(crate::protocol_runtime::udp::PacketPathCarrierDescriptor {
            cache_key: crate::protocol_runtime::udp::hysteria2_udp_cache_key(
                tag,
                server,
                *port,
                password,
                *client_fingerprint,
            ),
            server: (*server).to_string(),
            port: *port,
        })
    }

    pub(super) fn udp_packet_path_carrier_snapshot_impl(
        &self,
        leaf: &ResolvedLeafOutbound<'_>,
    ) -> Option<crate::protocol_runtime::udp::UdpPacketPathCarrier> {
        use crate::protocol_runtime::udp::UdpPacketPathCarrier;

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
        Some(UdpPacketPathCarrier::Hysteria2 {
            cache_key: crate::protocol_runtime::udp::hysteria2_udp_cache_key(
                tag,
                server,
                *port,
                password,
                *client_fingerprint,
            ),
            tag: (*tag).to_string(),
            server: (*server).to_string(),
            port: *port,
            password: (*password).to_string(),
            client_fingerprint: (*client_fingerprint).map(|value| value.to_string()),
        })
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
        crate::protocol_runtime::udp::build_hysteria2_packet_path(
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
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        let sent = protocol_state
            .start_hysteria2_udp_flow(crate::protocol_runtime::udp::Hysteria2UdpFlowRequest {
                chain_tasks,
                session,
                server,
                port: *port,
                password,
                client_fingerprint: *client_fingerprint,
                payload,
            })
            .await
            .map_err(|f: FlowFailure| FlowFailure {
                stage: f.stage,
                error: f.error,
                upstream: f.upstream,
            })?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::Datagram {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                protocol: ProtocolUdpFlowSnapshot::Hysteria2 {
                    password: (*password).to_string(),
                    client_fingerprint: (*client_fingerprint).map(|s| s.to_string()),
                },
            }),
            tx_bytes: sent as u64,
        })
    }
}
