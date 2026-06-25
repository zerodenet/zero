use tokio::sync::broadcast;
use tokio::task::JoinSet;
use zero_engine::EngineError;

use super::super::ChainTask;
use super::model::TrojanPacket;

pub(super) fn response_channel() -> broadcast::Sender<TrojanPacket> {
    let (tx, _) = broadcast::channel::<TrojanPacket>(32);
    tx
}

pub(super) fn spawn_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    recv_tx: broadcast::Sender<TrojanPacket>,
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
