use super::bridge;
use super::model::TrojanPacket;
use super::socket::{ReadOnlySocket, WriteOnlySocket};
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, TcpRelayStream};
use tokio::sync::{broadcast, mpsc};
use zero_core::Session;
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<TrojanPacket>,
    pub(super) recv_tx: broadcast::Sender<TrojanPacket>,
}

pub(super) async fn spawn_packet_stream(
    _proxy: &Proxy,
    session: &Session,
    stream: TcpRelayStream,
    password: &str,
) -> Result<PacketStream, EngineError> {
    let mut metered = MeteredStream::new(stream);
    trojan::establish_udp_packet_tunnel(&mut metered, session, password).await?;

    let (read_half, write_half) = tokio::io::split(metered.into_inner());
    let (send_tx, send_rx) = mpsc::channel::<TrojanPacket>(32);
    let recv_tx = bridge::response_channel();

    spawn_send_task(send_rx, WriteOnlySocket(write_half));
    spawn_recv_task(ReadOnlySocket(read_half), recv_tx.clone());

    Ok(PacketStream { send_tx, recv_tx })
}

fn spawn_send_task(mut send_rx: mpsc::Receiver<TrojanPacket>, mut send_stream: WriteOnlySocket) {
    tokio::spawn(async move {
        while let Some(packet) = send_rx.recv().await {
            if trojan::write_udp_response(
                &mut send_stream,
                &packet.target,
                packet.port,
                &packet.payload,
            )
            .await
            .is_err()
            {
                break;
            }
        }
    });
}

fn spawn_recv_task(mut recv_stream: ReadOnlySocket, recv_tx: broadcast::Sender<TrojanPacket>) {
    tokio::spawn(async move {
        while let Ok(packet) = trojan::read_inbound_udp_packet(&mut recv_stream).await {
            let packet = TrojanPacket {
                target: packet.target,
                port: packet.port,
                payload: packet.payload,
            };
            if recv_tx.send(packet).is_err() {
                break;
            }
        }
    });
}
