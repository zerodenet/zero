use super::{bridge, codec};
use crate::transport::TcpRelayStream;
use mieru::MieruOutbound;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, Mutex};

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<Vec<u8>>,
    pub(super) recv_tx: bridge::ResponseSender,
}

pub(super) fn spawn_packet_stream(stream: TcpRelayStream, outbound: MieruOutbound) -> PacketStream {
    let (send_tx, send_rx) = mpsc::channel::<Vec<u8>>(32);
    let recv_tx = bridge::response_channel();

    let shared_outbound = Arc::new(Mutex::new(outbound));
    let (read_half, write_half) = tokio::io::split(stream);

    spawn_send_task(shared_outbound.clone(), send_rx, write_half);
    spawn_recv_task(shared_outbound, read_half, recv_tx.clone());

    PacketStream { send_tx, recv_tx }
}

fn spawn_send_task(
    outbound: Arc<Mutex<MieruOutbound>>,
    mut send_rx: mpsc::Receiver<Vec<u8>>,
    mut write_half: tokio::io::WriteHalf<TcpRelayStream>,
) {
    tokio::spawn(async move {
        while let Some(payload) = send_rx.recv().await {
            let encrypted = {
                let mut ob = outbound.lock().await;
                match ob.encrypt_client_data(&payload) {
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
    outbound: Arc<Mutex<MieruOutbound>>,
    mut read_half: tokio::io::ReadHalf<TcpRelayStream>,
    recv_tx: bridge::ResponseSender,
) {
    tokio::spawn(async move {
        let mut raw = Vec::new();
        loop {
            let mut scratch = [0u8; 4096];
            match read_half.read(&mut scratch).await {
                Ok(0) => break,
                Ok(n) => raw.extend_from_slice(&scratch[..n]),
                Err(_) => break,
            }
            loop {
                let decrypted = {
                    let mut ob = outbound.lock().await;
                    ob.decrypt_server_data_with_consumed(&raw)
                };
                match decrypted {
                    Ok((segment, consumed)) => {
                        raw.drain(..consumed);
                        if !segment.payload.is_empty() {
                            if let Ok(packet) = codec::decode_packet(&segment.payload) {
                                if recv_tx
                                    .send((packet.target, packet.port, packet.payload))
                                    .is_err()
                                {
                                    return;
                                }
                            }
                        }
                    }
                    Err(zero_core::Error::Protocol("mieru: need more data")) => break,
                    Err(_) => return,
                }
            }
        }
    });
}
