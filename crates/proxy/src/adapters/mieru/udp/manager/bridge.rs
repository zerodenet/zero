use tokio::task::JoinSet;
use zero_engine::EngineError;

use crate::runtime::udp_flow::packet_path::ChainTask;

pub(super) fn spawn_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    mut recv_rx: mieru::MieruUdpFlowResponseReceiver,
    session_id: u64,
) {
    chain_tasks.spawn(async move {
        let (target, port, payload) = recv_rx
            .recv()
            .await
            .map_err(|_| EngineError::Io(std::io::Error::other("mieru upstream closed")))?;
        Ok((target, port, payload, Some(session_id)))
    });
}
