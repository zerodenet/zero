use super::bridge;
use super::model::MieruPacket;
use crate::transport::TcpRelayStream;
use mieru::MieruUdpFlowIo;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<MieruPacket>,
    pub(super) recv_tx: bridge::ResponseSender,
}

pub(super) fn spawn_packet_stream(stream: TcpRelayStream, flow_io: MieruUdpFlowIo) -> PacketStream {
    let (send_tx, send_rx) = mpsc::channel::<MieruPacket>(32);
    let recv_tx = bridge::response_channel();

    let shared_flow_io = Arc::new(Mutex::new(flow_io));
    let (read_half, write_half) = tokio::io::split(stream);

    spawn_send_task(shared_flow_io.clone(), send_rx, write_half);
    spawn_recv_task(shared_flow_io, read_half, recv_tx.clone());

    PacketStream { send_tx, recv_tx }
}

fn spawn_send_task(
    flow_io: Arc<Mutex<MieruUdpFlowIo>>,
    mut send_rx: mpsc::Receiver<MieruPacket>,
    mut write_half: tokio::io::WriteHalf<TcpRelayStream>,
) {
    tokio::spawn(async move {
        while let Some(packet) = send_rx.recv().await {
            let encrypted = {
                let mut io = flow_io.lock().await;
                match io.encrypt_packet(&packet.target, packet.port, &packet.payload) {
                    Ok(encrypted) => encrypted,
                    Err(_) => break,
                }
            };
            if write_half.write_all(&encrypted).await.is_err() {
                break;
            }
            if write_half.flush().await.is_err() {
                break;
            }
        }
    });
}

fn spawn_recv_task(
    flow_io: Arc<Mutex<MieruUdpFlowIo>>,
    mut read_half: tokio::io::ReadHalf<TcpRelayStream>,
    recv_tx: bridge::ResponseSender,
) {
    tokio::spawn(async move {
        loop {
            let mut scratch = [0u8; 4096];
            match read_half.read(&mut scratch).await {
                Ok(0) => break,
                Ok(n) => {
                    let mut io = flow_io.lock().await;
                    io.push_encrypted_response(&scratch[..n]);
                }
                Err(_) => break,
            }
            loop {
                let packet = {
                    let mut io = flow_io.lock().await;
                    io.next_packet()
                };
                match packet {
                    Ok(Some(packet)) => {
                        if recv_tx
                            .send((packet.target, packet.port, packet.payload))
                            .is_err()
                        {
                            return;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => return,
                }
            }
        }
    });
}
