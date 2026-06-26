use crate::transport::{
    establish_mieru_udp_flow_stream, MieruUdpFlowStreamRequest, TcpRelayStream,
};
use mieru::MieruUdpFlowPacket;
use tokio::sync::{broadcast, mpsc};
use zero_core::Address;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<MieruUdpFlowPacket>,
    pub(super) recv_tx: broadcast::Sender<(Address, u16, Vec<u8>)>,
}

pub(super) async fn spawn_packet_stream(
    stream: TcpRelayStream,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let stream =
        establish_mieru_udp_flow_stream(MieruUdpFlowStreamRequest { stream, resume }).await?;

    Ok(PacketStream {
        send_tx: stream.send_tx,
        recv_tx: stream.recv_tx,
    })
}
