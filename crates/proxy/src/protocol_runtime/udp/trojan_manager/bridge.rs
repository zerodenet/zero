use tokio::sync::broadcast;
use tokio::task::JoinSet;
use zero_core::UdpFlowPacket;
use zero_engine::EngineError;

use crate::runtime::udp_flow::packet_path::ChainTask;

pub(super) fn spawn_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    recv_tx: broadcast::Sender<UdpFlowPacket>,
    session_id: u64,
) {
    let mut recv_rx = recv_tx.subscribe();
    chain_tasks.spawn(async move {
        let packet = recv_rx
            .recv()
            .await
            .map_err(|_| EngineError::Io(std::io::Error::other("trojan upstream closed")))?;
        Ok((packet.target, packet.port, packet.payload, Some(session_id)))
    });
}
