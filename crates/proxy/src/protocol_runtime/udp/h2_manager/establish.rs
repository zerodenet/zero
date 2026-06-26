use super::super::packet_path_traits::UdpPacketRef;
use super::super::ChainTask;
use super::super::H2UdpPeer;
use super::{bridge, stream};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use zero_core::{Address, Error};
use zero_engine::EngineError;
use zero_traits::DatagramCodec;

pub(super) async fn upstream(
    chain_tasks: &mut JoinSet<ChainTask>,
    session_id: u64,
    peer: &H2UdpPeer<'_>,
    codec: Arc<dyn DatagramCodec<Address, Error = Error>>,
    initial_packet: UdpPacketRef<'_>,
) -> Result<mpsc::Sender<Vec<u8>>, EngineError> {
    let stream::PacketStream { send_tx, recv_tx } =
        stream::establish(peer, initial_packet, codec).await?;

    bridge::spawn_response_bridge(chain_tasks, recv_tx, session_id);

    Ok(send_tx)
}
