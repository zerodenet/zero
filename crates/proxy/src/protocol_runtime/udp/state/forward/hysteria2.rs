use tokio::task::JoinSet;

use super::super::ProtocolUdpState;
use crate::protocol_runtime::udp::{ChainTask, FlowFailure, H2SendExisting};
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;

pub(super) struct ExistingFlow<'a> {
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) password: &'a str,
    pub(super) client_fingerprint: Option<&'a str>,
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
            password: existing.password,
            client_fingerprint: existing.client_fingerprint,
            target: &flow.session.target,
            target_port: flow.session.port,
            payload: existing.payload,
        })
        .await
}
