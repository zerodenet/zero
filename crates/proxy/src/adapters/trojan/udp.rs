use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::trojan::TrojanAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_dispatch::{ManagedProtocolUdpSend, ManagedUdpOutboundKind};
use crate::runtime::udp_flow::managed::ManagedStreamFlowHandler;
use crate::runtime::udp_flow::managed::{ManagedUdpFlowKind, ManagedUdpFlowResume};
use crate::runtime::Proxy;

mod manager;

pub(crate) fn managed_stream_handler() -> Box<dyn ManagedStreamFlowHandler> {
    Box::new(manager::TrojanChainManager::new())
}

impl TrojanAdapter {
    pub(super) async fn start_udp_flow_impl(
        &self,
        dispatch: &mut UdpDispatch,
        proxy: &Proxy,
        session: &Session,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
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
                resume: ManagedUdpFlowResume::new(trojan::TrojanUdpFlowResume::new(
                    password,
                    *sni,
                    *insecure,
                    *client_fingerprint,
                    false,
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
        proxy: &Proxy,
        session: &Session,
        carrier: crate::transport::RelayCarrier,
        leaf: &ResolvedLeafOutbound<'_>,
        payload: &[u8],
    ) -> Result<FlowStartResult, FlowFailure> {
        let ResolvedLeafOutbound::Trojan {
            tag,
            server,
            port,
            password,
            sni,
            insecure,
            client_fingerprint,
        } = leaf
        else {
            return Err(unreachable_udp_leaf(self.name(), leaf));
        };
        dispatch
            .start_tracked_managed_protocol_udp(ManagedProtocolUdpSend {
                proxy: Some(proxy),
                tag,
                session,
                carrier: Some(carrier),
                tls_server_name: None,
                server,
                port: *port,
                resume: ManagedUdpFlowResume::new(trojan::TrojanUdpFlowResume::new(
                    password,
                    *sni,
                    *insecure,
                    *client_fingerprint,
                    true,
                )),
                payload,
                kind: ManagedUdpFlowKind::RelayStream,
                outbound: ManagedUdpOutboundKind::StreamPacket,
            })
            .await
    }
}
