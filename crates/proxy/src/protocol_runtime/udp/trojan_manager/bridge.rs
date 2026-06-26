use tokio::sync::broadcast;
use tokio::task::JoinSet;
use zero_engine::EngineError;

use super::super::ChainTask;

pub(super) fn response_channel() -> broadcast::Sender<trojan::TrojanUdpPacket> {
    let (tx, _) = broadcast::channel::<trojan::TrojanUdpPacket>(32);
    tx
}

pub(super) fn spawn_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    recv_tx: broadcast::Sender<trojan::TrojanUdpPacket>,
    session_id: u64,
) {
    let mut recv_rx = recv_tx.subscribe();
    chain_tasks.spawn(async move {
        let packet = recv_rx
            .recv()
            .await
            .map_err(|_| EngineError::Io(std::io::Error::other("trojan upstream closed")))?;
        let (target, port, payload) = packet.into_parts();
        Ok((target, port, payload, Some(session_id)))
    });
}
