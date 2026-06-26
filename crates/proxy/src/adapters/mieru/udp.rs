use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::mieru::MieruAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::protocol_runtime::udp::{ManagedUdpFlowKind, ProtocolUdpFlowResume};
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_dispatch::{ManagedProtocolUdpSend, ManagedUdpOutboundKind};
use crate::runtime::Proxy;

impl MieruAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Mieru {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        dispatch
            .start_tracked_managed_protocol_udp(ManagedProtocolUdpSend {
                proxy: Some(proxy),
                tag,
                session,
                carrier: None,
                tls_server_name: None,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::new(mieru::MieruUdpFlowResume::new(
                    username, password, false,
                )),
                payload,
                kind: ManagedUdpFlowKind::StreamPacket,
                outbound: ManagedUdpOutboundKind::StreamPacket,
            })
            .await
    }

    pub(super) async fn start_udp_relay_final_hop_impl(
        &self,
        dispatch: &mut UdpDispatch,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Mieru {
            tag,
            server,
            port,
            username,
            password,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        dispatch
            .start_tracked_managed_protocol_udp(ManagedProtocolUdpSend {
                proxy: None,
                tag,
                session,
                carrier: Some(carrier),
                tls_server_name: None,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::new(mieru::MieruUdpFlowResume::new(
                    username, password, true,
                )),
                payload,
                kind: ManagedUdpFlowKind::RelayStream,
                outbound: ManagedUdpOutboundKind::StreamPacket,
            })
            .await
    }
}
