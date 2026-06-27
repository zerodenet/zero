use tokio::task::JoinSet;
use zero_engine::EngineError;

use crate::runtime::udp_flow::packet_path::ChainTask;

pub(super) fn spawn_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    mut recv_rx: hysteria2::Hysteria2UdpFlowResponseReceiver,
    session_id: u64,
) {
    chain_tasks.spawn(async move {
        match recv_rx.recv().await {
            Ok((target, port, payload)) => Ok((target, port, payload, Some(session_id))),
            Err(_) => Err(EngineError::Io(std::io::Error::other("h2 upstream closed"))),
        }
    });
}
