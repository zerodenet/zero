use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::trojan::TrojanAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
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
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        let sent = protocol_state
            .start_trojan_udp_flow(crate::protocol_runtime::udp::TrojanUdpFlowRequest {
                chain_tasks,
                proxy,
                session,
                server,
                port: *port,
                password,
                sni: *sni,
                insecure: *insecure,
                client_fingerprint: *client_fingerprint,
                relay_chain: false,
                payload,
            })
            .await
            .map_err(|f: FlowFailure| FlowFailure {
                stage: f.stage,
                error: f.error,
                upstream: f.upstream,
            })?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::StreamPacket {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                protocol: ProtocolUdpFlowSnapshot::Trojan {
                    password: (*password).to_string(),
                    sni: (*sni).map(|s| s.to_string()),
                    insecure: *insecure,
                    client_fingerprint: (*client_fingerprint).map(|s| s.to_string()),
                    relay_chain: false,
                },
            }),
            tx_bytes: sent as u64,
        })
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
        let (protocol_state, chain_tasks) = dispatch.protocol_parts();
        let sent = protocol_state
            .start_trojan_udp_relay_flow(crate::protocol_runtime::udp::TrojanUdpRelayFlowRequest {
                chain_tasks,
                proxy,
                session,
                carrier,
                server,
                port: *port,
                password,
                sni: *sni,
                insecure: *insecure,
                client_fingerprint: *client_fingerprint,
                payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::StreamPacket {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                protocol: ProtocolUdpFlowSnapshot::Trojan {
                    password: (*password).to_string(),
                    sni: (*sni).map(|s| s.to_string()),
                    insecure: *insecure,
                    client_fingerprint: (*client_fingerprint).map(|s| s.to_string()),
                    relay_chain: true,
                },
            }),
            tx_bytes: sent as u64,
        })
    }
}
