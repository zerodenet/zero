use zero_core::Session;
use zero_engine::ResolvedLeafOutbound;

use crate::adapters::common::unreachable_udp_leaf;
use crate::adapters::mieru::MieruAdapter;
use crate::protocol_adapter::ProtocolSupportCapability;
use crate::protocol_runtime::udp::mieru_flow::{MieruDatagramSend, MieruRelaySend};
use crate::protocol_runtime::udp::ProtocolUdpFlowResume;
use crate::runtime::udp_dispatch::{FlowFailure, FlowStartResult, UdpDispatch};
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
            .start_mieru_datagram_flow(MieruDatagramSend {
                proxy,
                tag,
                session,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::Mieru(mieru::MieruUdpFlowResume::new(
                    username, password, false,
                )),
                payload,
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
            .start_mieru_relay_flow(MieruRelaySend {
                tag,
                session,
                carrier,
                server,
                port: *port,
                resume: ProtocolUdpFlowResume::Mieru(mieru::MieruUdpFlowResume::new(
                    username, password, true,
                )),
                payload,
            })
            .await
    }
}
