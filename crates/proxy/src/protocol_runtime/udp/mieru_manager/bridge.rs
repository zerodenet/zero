use tokio::sync::broadcast;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use crate::runtime::udp_flow::packet_path::ChainTask;

pub(super) type ResponseItem = (Address, u16, Vec<u8>);
pub(super) type ResponseSender = broadcast::Sender<ResponseItem>;

pub(super) fn spawn_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    recv_tx: ResponseSender,
    session_id: u64,
) {
    let mut recv_rx = recv_tx.subscribe();
    chain_tasks.spawn(async move {
        let (target, port, payload) = recv_rx
            .recv()
            .await
            .map_err(|_| EngineError::Io(std::io::Error::other("mieru upstream closed")))?;
        Ok((target, port, payload, Some(session_id)))
    });
}
