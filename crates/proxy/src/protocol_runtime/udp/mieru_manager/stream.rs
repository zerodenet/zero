use super::bridge;
use crate::transport::TcpRelayStream;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use zero_core::{Address, UdpFlowPacket};
use zero_engine::EngineError;

pub(super) struct PacketStream {
    pub(super) sender: MieruFlowSender,
    pub(super) recv_tx: bridge::ResponseSender,
}

#[derive(Clone)]
pub(super) struct MieruFlowSender {
    send_tx: mpsc::Sender<UdpFlowPacket>,
}

impl MieruFlowSender {
    pub(super) async fn send(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, zero_core::Error> {
        let packet = UdpFlowPacket::from_parts(target, port, payload);
        let packet_len = packet.payload.len();
        self.send_tx
            .send(packet)
            .await
            .map_err(|_| zero_core::Error::Io("mieru udp flow closed"))?;
        Ok(packet_len)
    }
}

pub(super) async fn spawn_packet_stream(
    mut stream: TcpRelayStream,
    resume: &mieru::MieruUdpFlowResume,
) -> Result<PacketStream, EngineError> {
    let flow_io = mieru::MieruUdpFlowIo::establish_with_resume(&mut stream, resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;
    Ok(spawn_udp_flow(stream, flow_io))
}

fn spawn_udp_flow(stream: TcpRelayStream, flow_io: mieru::MieruUdpFlowIo) -> PacketStream {
    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (recv_tx, _) = tokio::sync::broadcast::channel::<bridge::ResponseItem>(32);
    spawn_udp_flow_task(stream, flow_io, send_rx, recv_tx.clone());
    PacketStream {
        sender: MieruFlowSender { send_tx },
        recv_tx,
    }
}

fn spawn_udp_flow_task(
    mut stream: TcpRelayStream,
    mut flow_io: mieru::MieruUdpFlowIo,
    mut send_rx: mpsc::Receiver<UdpFlowPacket>,
    responses: bridge::ResponseSender,
) {
    tokio::spawn(async move {
        let mut scratch = [0u8; 4096];
        loop {
            tokio::select! {
                to_send = send_rx.recv() => {
                    match to_send {
                        Some(packet) => {
                            if flow_io.write_flow_packet(
                                &mut stream,
                                &packet.target,
                                packet.port,
                                &packet.payload,
                            ).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                read = stream.read(&mut scratch) => {
                    match read {
                        Ok(0) => break,
                        Ok(n) => {
                            let packets = match flow_io.decode_encrypted_response(&scratch[..n]) {
                                Ok(packets) => packets,
                                Err(_) => return,
                            };
                            for packet in packets {
                                let _ = responses.send(packet.into_parts());
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    });
}
