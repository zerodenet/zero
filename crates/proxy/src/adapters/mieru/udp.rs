use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::mieru::MieruAdapter;
use crate::protocol_adapter::ProtocolAdapter;
use crate::protocol_runtime::udp::ProtocolUdpFlowSnapshot;
use crate::runtime::udp_dispatch::{
    FlowFailure, FlowStartResult, MieruDatagramSend, MieruRelaySend, UdpDispatch,
};
use crate::runtime::udp_flow::outbound::UdpFlowOutbound;
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
        let sent = dispatch
            .send_mieru_datagram(MieruDatagramSend {
                proxy,
                session,
                server,
                port: *port,
                username,
                password,
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
                protocol: ProtocolUdpFlowSnapshot::Mieru {
                    username: (*username).to_string(),
                    password: (*password).to_string(),
                    relay_chain: false,
                },
            }),
            tx_bytes: sent as u64,
        })
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
        let sent = dispatch
            .send_mieru_relay(MieruRelaySend {
                session,
                carrier,
                server,
                port: *port,
                username,
                password,
                payload,
            })
            .await?;
        Ok(FlowStartResult::Flow {
            outbound: Box::new(UdpFlowOutbound::StreamPacket {
                tag: (*tag).to_string(),
                server: (*server).to_string(),
                port: *port,
                protocol: ProtocolUdpFlowSnapshot::Mieru {
                    username: (*username).to_string(),
                    password: (*password).to_string(),
                    relay_chain: true,
                },
            }),
            tx_bytes: sent as u64,
        })
    }
}
