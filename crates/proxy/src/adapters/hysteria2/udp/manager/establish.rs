use super::model::H2UdpPeer;
use super::{bridge, stream};
use crate::runtime::udp_flow::packet_path::ChainTask;
use crate::runtime::udp_flow::packet_path::UdpPacketRef;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use zero_core::UdpFlowPacket;
use zero_engine::EngineError;

pub(super) async fn upstream(
    chain_tasks: &mut JoinSet<ChainTask>,
    session_id: u64,
    peer: &H2UdpPeer<'_>,
    resume: hysteria2::Hysteria2UdpFlowResume,
    initial_packet: UdpPacketRef<'_>,
) -> Result<mpsc::Sender<UdpFlowPacket>, EngineError> {
    let stream::PacketStream { send_tx, recv_tx } =
        stream::establish(peer, initial_packet, resume).await?;

    bridge::spawn_response_bridge(chain_tasks, recv_tx, session_id);

    Ok(send_tx)
}
