use super::bridge;
use super::socket::{ReadOnlySocket, WriteOnlySocket};
use crate::transport::TcpRelayStream;
use mieru::{MieruUdpFlowIo, MieruUdpFlowPacket};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<MieruUdpFlowPacket>,
    pub(super) recv_tx: bridge::ResponseSender,
}

pub(super) fn spawn_packet_stream(stream: TcpRelayStream, flow_io: MieruUdpFlowIo) -> PacketStream {
    let (send_tx, send_rx) = mpsc::channel::<MieruUdpFlowPacket>(32);
    let recv_tx = bridge::response_channel();

    let shared_flow_io = Arc::new(Mutex::new(flow_io));
    let (read_half, write_half) = tokio::io::split(stream);

    spawn_send_task(shared_flow_io.clone(), send_rx, WriteOnlySocket(write_half));
    spawn_recv_task(shared_flow_io, ReadOnlySocket(read_half), recv_tx.clone());

    PacketStream { send_tx, recv_tx }
}

fn spawn_send_task(
    flow_io: Arc<Mutex<MieruUdpFlowIo>>,
    mut send_rx: mpsc::Receiver<MieruUdpFlowPacket>,
    mut write_stream: WriteOnlySocket,
) {
    tokio::spawn(async move {
        while let Some(packet) = send_rx.recv().await {
            let mut io = flow_io.lock().await;
            if io.write_packet(&mut write_stream, &packet).await.is_err() {
                break;
            }
        }
    });
}

fn spawn_recv_task(
    flow_io: Arc<Mutex<MieruUdpFlowIo>>,
    mut read_stream: ReadOnlySocket,
    recv_tx: bridge::ResponseSender,
) {
    tokio::spawn(async move {
        let mut scratch = [0u8; 4096];
        loop {
            let packets = {
                let mut io = flow_io.lock().await;
                match io.read_packets(&mut read_stream, &mut scratch).await {
                    Ok(Some(packets)) => packets,
                    Ok(None) => break,
                    Err(_) => break,
                }
            };

            for packet in packets {
                if recv_tx.send(packet.into_parts()).is_err() {
                    return;
                }
            }
        }
    });
}
