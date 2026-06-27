use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use tokio::sync::{broadcast, mpsc};
use zero_core::{Session, UdpFlowPacket};
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<UdpFlowPacket>,
    pub(super) recv_tx: broadcast::Sender<UdpFlowPacket>,
}

pub(super) async fn spawn_packet_stream(
    _proxy: &Proxy,
    session: &Session,
    mut stream: TcpRelayStream,
    resume: &trojan::TrojanUdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let flow_io = trojan::TrojanUdpFlowIo;
    flow_io
        .establish_with_resume(&mut stream, session, resume)
        .await?;

    let trojan::TrojanUdpFlowHandle { send_tx, recv_tx } = trojan::spawn_udp_flow(stream);

    Ok(PacketStream { send_tx, recv_tx })
}
