use tokio::task::JoinSet;

use super::super::ProtocolUdpState;
use crate::protocol_runtime::udp::packet_path_traits::{UdpFlowContext, UdpPacketRef};
use crate::protocol_runtime::udp::ss_manager::model::SsSendExisting;
use crate::protocol_runtime::udp::{
    ChainTask, FlowFailure, ProtocolUdpFlowSnapshot, SendWithSnapshotRequest, UdpPacketPathCarrier,
};
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

pub(super) struct ExistingFlow<'a> {
    pub(super) tag: &'a str,
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) password: &'a str,
    pub(super) datagram_cache_key: &'a str,
    pub(super) cipher_kind: shadowsocks::CipherKind,
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
            .send_with_snapshot(SendWithSnapshotRequest {
                ctx: UdpFlowContext {
                    chain_tasks,
                    session_id: flow.session.id,
                },
                carrier,
                datagram_tag: existing.tag,
                datagram_server: existing.server,
                datagram_port: existing.port,
                datagram_cache_key: existing.datagram_cache_key,
                packet_ref: UdpPacketRef {
                    target: &flow.session.target,
                    port: flow.session.port,
                    payload: existing.payload,
                },
            })
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
                cipher: existing.cipher_kind,
                target: &flow.session.target,
                target_port: flow.session.port,
                payload: existing.payload,
            })
            .await
    }
}

pub(super) async fn forward_if_matches(
    state: &mut ProtocolUdpState,
    chain_tasks: &mut JoinSet<ChainTask>,
    proxy: &Proxy,
    flow: &UdpFlowSnapshot,
    snapshot: &ProtocolUdpFlowSnapshot,
    payload: &[u8],
) -> Option<Result<usize, FlowFailure>> {
    let ProtocolUdpFlowSnapshot::Shadowsocks {
        password,
        datagram_cache_key,
        cipher_kind,
        packet_path_carrier,
    } = snapshot
    else {
        return None;
    };

    let upstream = flow
        .outbound
        .upstream()
        .expect("protocol flow should have upstream");

    Some(
        forward(
            state,
            chain_tasks,
            proxy,
            flow,
            ExistingFlow {
                tag: flow.outbound.tag(),
                server: upstream.server,
                port: upstream.port,
                password,
                datagram_cache_key,
                cipher_kind: *cipher_kind,
                packet_path_carrier: packet_path_carrier.as_ref(),
                payload,
            },
        )
        .await,
    )
}
