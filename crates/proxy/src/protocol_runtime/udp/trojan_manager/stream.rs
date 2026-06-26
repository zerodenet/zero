use crate::runtime::Proxy;
use crate::transport::{
    establish_trojan_udp_flow_stream, TcpRelayStream, TrojanUdpFlowStreamRequest,
};
use tokio::sync::{broadcast, mpsc};
use zero_core::Session;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<trojan::TrojanUdpPacket>,
    pub(super) recv_tx: broadcast::Sender<trojan::TrojanUdpPacket>,
}

pub(super) async fn spawn_packet_stream(
    _proxy: &Proxy,
    session: &Session,
    stream: TcpRelayStream,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let stream = establish_trojan_udp_flow_stream(TrojanUdpFlowStreamRequest {
        stream,
        session,
        resume,
    })
    .await?;

    Ok(PacketStream {
        send_tx: stream.send_tx,
        recv_tx: stream.recv_tx,
    })
}
