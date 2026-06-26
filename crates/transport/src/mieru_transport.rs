//! Mieru UDP flow transport helpers.

use std::io;
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{broadcast, mpsc, Mutex};
use zero_core::{Address, UdpFlowPacket};
use zero_engine::EngineError;
use zero_platform_tokio::TcpRelayStream;
use zero_traits::AsyncSocket;

pub type MieruUdpResponse = (Address, u16, Vec<u8>);

pub struct MieruUdpFlowStream {
    pub send_tx: mpsc::Sender<UdpFlowPacket>,
    pub recv_tx: broadcast::Sender<MieruUdpResponse>,
}

pub struct MieruUdpFlowStreamRequest<'a> {
    pub stream: TcpRelayStream,
    pub resume: &'a mieru::MieruUdpFlowResume,
}

pub async fn establish_mieru_udp_flow_stream(
    request: MieruUdpFlowStreamRequest<'_>,
) -> Result<MieruUdpFlowStream, EngineError> {
    let mut stream = request.stream;
    let flow_io = mieru::MieruUdpFlowIo::establish_with_resume(&mut stream, request.resume)
        .await
        .map_err(|error| {
            EngineError::Io(std::io::Error::other(format!(
                "mieru udp associate: {error}"
            )))
        })?;

    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (recv_tx, _) = broadcast::channel::<MieruUdpResponse>(32);
    let shared_flow_io = Arc::new(Mutex::new(flow_io));
    let (read_half, write_half) = tokio::io::split(stream);

    spawn_mieru_udp_send_task(shared_flow_io.clone(), send_rx, WriteOnlySocket(write_half));
    spawn_mieru_udp_recv_task(shared_flow_io, ReadOnlySocket(read_half), recv_tx.clone());

    Ok(MieruUdpFlowStream { send_tx, recv_tx })
}

fn spawn_mieru_udp_send_task(
    flow_io: Arc<Mutex<mieru::MieruUdpFlowIo>>,
    mut send_rx: mpsc::Receiver<UdpFlowPacket>,
    mut write_stream: WriteOnlySocket,
) {
    tokio::spawn(async move {
        while let Some(packet) = send_rx.recv().await {
            let packet = mieru::udp_flow_packet(&packet.target, packet.port, &packet.payload);
            let mut io = flow_io.lock().await;
            if io.write_packet(&mut write_stream, &packet).await.is_err() {
                break;
            }
        }
    });
}

fn spawn_mieru_udp_recv_task(
    flow_io: Arc<Mutex<mieru::MieruUdpFlowIo>>,
    mut read_stream: ReadOnlySocket,
    recv_tx: broadcast::Sender<MieruUdpResponse>,
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

struct ReadOnlySocket(ReadHalf<TcpRelayStream>);

impl AsyncSocket for ReadOnlySocket {
    type Error = io::Error;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.0.read(buf).await
    }

    async fn write_all(&mut self, _buf: &[u8]) -> Result<(), Self::Error> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "read-only socket cannot write",
        ))
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct WriteOnlySocket(WriteHalf<TcpRelayStream>);

impl AsyncSocket for WriteOnlySocket {
    type Error = io::Error;

    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "write-only socket cannot read",
        ))
    }

    async fn write_all(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.0.write_all(buf).await?;
        self.0.flush().await
    }

    async fn shutdown(&mut self) -> Result<(), Self::Error> {
        self.0.shutdown().await
    }
}
