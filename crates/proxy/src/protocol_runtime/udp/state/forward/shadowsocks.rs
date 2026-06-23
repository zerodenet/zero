use tokio::task::JoinSet;

use super::super::ProtocolUdpState;
use crate::protocol_runtime::udp::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use crate::protocol_runtime::udp::{ChainTask, FlowFailure, SsSendExisting, UdpPacketPathCarrier};
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

pub(super) struct ExistingFlow<'a> {
    pub(super) tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) password: &'a str,
    pub(super) cipher: &'a str,
    pub(super) packet_path_carrier: Option<&'a UdpPacketPathCarrier>,
    pub(super) payload: &'a [u8],
}

pub(super) async fn forward(
    state: &mut ProtocolUdpState,
    chain_tasks: &mut JoinSet<ChainTask>,
    proxy: &Proxy,
    flow: &UdpFlowSnapshot,
    existing: ExistingFlow<'_>,
) -> Result<usize, FlowFailure> {
    if let Some(carrier) = existing.packet_path_carrier {
        state
            .packet_path
            .send_with_snapshot(
                UdpFlowContext {
                    chain_tasks,
                    session_id: flow.session.id,
                },
                carrier,
                existing.tag,
                existing.server,
                existing.port,
                existing.password,
                existing.cipher,
                UdpPacketRef {
                    target: &flow.session.target,
                    port: flow.session.port,
                    payload: existing.payload,
                },
            )
            .await
    } else {
        state
            .shadowsocks
            .send_existing(SsSendExisting {
                chain_tasks,
                session_id: flow.session.id,
                proxy,
                server: existing.server,
                port: existing.port,
                password: existing.password,
                cipher: existing.cipher,
                target: &flow.session.target,
                target_port: flow.session.port,
                payload: existing.payload,
            })
            .await
    }
}
