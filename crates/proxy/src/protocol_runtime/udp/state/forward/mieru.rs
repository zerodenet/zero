use tokio::task::JoinSet;

use super::super::ProtocolUdpState;
use crate::protocol_runtime::udp::mieru_manager::model::MieruSendExisting;
use crate::protocol_runtime::udp::{ChainTask, FlowFailure};
use crate::runtime::udp_flow::sessions::UdpFlowSnapshot;
use crate::runtime::Proxy;

pub(super) struct ExistingFlow<'a> {
    pub(super) server: &'a str,
    pub(super) port: u16,
    pub(super) username: &'a str,
    pub(super) password: &'a str,
    pub(super) relay_chain: bool,
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
            username: existing.username,
            password: existing.password,
            relay_chain: existing.relay_chain,
            target: &flow.session.target,
            target_port: flow.session.port,
            payload: existing.payload,
        })
        .await
}
