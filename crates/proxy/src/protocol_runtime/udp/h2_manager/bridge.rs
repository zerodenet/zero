use tokio::sync::broadcast;
use tokio::task::JoinSet;
use zero_core::Address;
use zero_engine::EngineError;

use super::super::ChainTask;

pub(super) type RecvItem = (Address, u16, Vec<u8>);

pub(super) type ResponseSender = broadcast::Sender<RecvItem>;

pub(super) fn spawn_response_bridge(
    chain_tasks: &mut JoinSet<ChainTask>,
    recv_tx: ResponseSender,
    session_id: u64,
) {
    let mut recv_rx = recv_tx.subscribe();
    chain_tasks.spawn(async move {
        match recv_rx.recv().await {
            Ok((target, port, payload)) => Ok((target, port, payload, Some(session_id))),
            Err(_) => Err(EngineError::Io(std::io::Error::other("h2 upstream closed"))),
        }
    });
}
