use tokio::task::JoinSet;

use super::super::ProtocolUdpState;
use crate::protocol_runtime::udp::mieru_manager::model::MieruSendExisting;
use crate::protocol_runtime::udp::{ChainTask, FlowFailure, ProtocolUdpFlowSnapshot};
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

pub(super) struct ExistingFlow<'a> {
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) resume: &'a mieru::MieruUdpFlowResume,
    pub(super) payload: &'a [u8],
}

pub(super) async fn forward(
    state: &mut ProtocolUdpState,
    chain_tasks: &mut JoinSet<ChainTask>,
    proxy: &Proxy,
    flow: &UdpFlowSnapshot,
    existing: ExistingFlow<'_>,
) -> Result<usize, FlowFailure> {
    state
        .mieru
        .send_existing(MieruSendExisting {
            chain_tasks,
            session_id: flow.session.id,
            proxy,
            session: &flow.session,
            server: existing.server,
            port: existing.port,
            username: existing.resume.username(),
            password: existing.resume.password(),
            relay_chain: existing.resume.relay_chain(),
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
    proxy: &Proxy,
    flow: &UdpFlowSnapshot,
    snapshot: &ProtocolUdpFlowSnapshot,
    payload: &[u8],
) -> Option<Result<usize, FlowFailure>> {
    let ProtocolUdpFlowSnapshot::Mieru { resume } = snapshot else {
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
                server: upstream.server,
                port: upstream.port,
                resume,
                payload,
            },
        )
        .await,
    )
}
