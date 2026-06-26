use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::trojan::TrojanAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::protocol_runtime::udp::trojan_flow::{TrojanDatagramSend, TrojanRelaySend};
use crate::protocol_runtime::udp::ProtocolUdpFlowResume;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::Proxy;

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
            .start_trojan_datagram_flow(TrojanDatagramSend {
                proxy,
                tag,
                session,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::Trojan(trojan::TrojanUdpFlowResume::new(
                    password,
                    *sni,
                    *insecure,
                    *client_fingerprint,
                    false,
                )),
                payload,
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
            .start_trojan_relay_flow(TrojanRelaySend {
                proxy,
                tag,
                session,
                carrier,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::Trojan(trojan::TrojanUdpFlowResume::new(
                    password,
                    *sni,
                    *insecure,
                    *client_fingerprint,
                    true,
                )),
                payload,
            })
            .await
    }
}
