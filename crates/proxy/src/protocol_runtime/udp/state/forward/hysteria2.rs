use tokio::task::JoinSet;

use super::super::ProtocolUdpState;
use crate::protocol_runtime::udp::h2_manager::model::H2SendExisting;
use crate::protocol_runtime::udp::{ChainTask, FlowFailure, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;

pub(super) struct ExistingFlow<'a> {
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: &'a hysteria2::Hysteria2UdpFlowResume,
    pub(super) payload: &'a [u8],
}

pub(super) async fn forward(
    state: &mut ProtocolUdpState,
    chain_tasks: &mut JoinSet<ChainTask>,
    flow: &UdpFlowSnapshot,
    existing: ExistingFlow<'_>,
) -> Result<usize, FlowFailure> {
    state
        .hysteria2
        .send_existing(H2SendExisting {
            chain_tasks,
            session_id: flow.session.id,
            server: existing.server,
            port: existing.port,
            password: existing.resume.password(),
            client_fingerprint: existing.resume.client_fingerprint(),
            codec: std::sync::Arc::new(existing.resume.codec()),
            target: &flow.session.target,
            target_port: flow.session.port,
            payload: existing.payload,
        })
        .await
}

pub(super) async fn forward_if_matches(
    state: &mut ProtocolUdpState,
    chain_tasks: &mut JoinSet<ChainTask>,
    flow: &UdpFlowSnapshot,
    snapshot: &ProtocolUdpFlowSnapshot,
    payload: &[u8],
) -> Option<Result<usize, FlowFailure>> {
    let ProtocolUdpFlowSnapshot::Hysteria2 { resume } = snapshot else {
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
            flow,
            ExistingFlow {
                server: upstream.server,
                port: upstream.port,
                resume,
                payload,
            },
        )
        .await,
    )
}
