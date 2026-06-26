use super::bridge;
use super::socket::{ReadOnlySocket, WriteOnlySocket};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};
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
    password: &str,
) -> Result<PacketStream, EngineError> {
    let mut metered = MeteredStream::new(stream);
    let flow_io = trojan::TrojanUdpFlowIo;
    flow_io.establish(&mut metered, session, password).await?;

    let (read_half, write_half) = tokio::io::split(metered.into_inner());
    let (send_tx, send_rx) = mpsc::channel::<trojan::TrojanUdpPacket>(32);
    let recv_tx = bridge::response_channel();

    spawn_send_task(send_rx, WriteOnlySocket(write_half));
    spawn_recv_task(ReadOnlySocket(read_half), recv_tx.clone());

    Ok(PacketStream { send_tx, recv_tx })
}

fn spawn_send_task(
    mut send_rx: mpsc::Receiver<trojan::TrojanUdpPacket>,
    mut send_stream: WriteOnlySocket,
) {
    tokio::spawn(async move {
        let flow_io = trojan::TrojanUdpFlowIo;
        while let Some(packet) = send_rx.recv().await {
            if flow_io
                .write_stream_packet(&mut send_stream, &packet)
                .await
                .is_err()
            {
                break;
            }
        }
    });
}

fn spawn_recv_task(
    mut recv_stream: ReadOnlySocket,
    recv_tx: broadcast::Sender<trojan::TrojanUdpPacket>,
) {
    tokio::spawn(async move {
        let flow_io = trojan::TrojanUdpFlowIo;
        while let Ok(packet) = flow_io.read_stream_packet(&mut recv_stream).await {
            if recv_tx.send(packet).is_err() {
                break;
            }
        }
    });
}
