use crate::runtime::Proxy;
use crate::transport::TcpRelayStream;
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{broadcast, mpsc};
use zero_core::{Session, UdpFlowPacket};
use zero_engine::EngineError;
use zero_traits::AsyncSocket;

pub(super) struct PacketStream {
    pub(super) send_tx: mpsc::Sender<UdpFlowPacket>,
    pub(super) recv_tx: broadcast::Sender<trojan::TrojanUdpPacket>,
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

    let (read_half, write_half) = tokio::io::split(stream);
    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (recv_tx, _) = broadcast::channel::<trojan::TrojanUdpPacket>(32);

    spawn_send_task(send_rx, WriteOnlySocket(write_half));
    spawn_recv_task(ReadOnlySocket(read_half), recv_tx.clone());

    Ok(PacketStream { send_tx, recv_tx })
}

fn spawn_send_task(mut send_rx: mpsc::Receiver<UdpFlowPacket>, mut send_stream: WriteOnlySocket) {
    tokio::spawn(async move {
        let flow_io = trojan::TrojanUdpFlowIo;
        while let Some(packet) = send_rx.recv().await {
            if flow_io
                .write_packet(
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

fn spawn_recv_task(
    mut recv_stream: ReadOnlySocket,
    recv_tx: broadcast::Sender<trojan::TrojanUdpPacket>,
) {
    tokio::spawn(async move {
        let flow_io = trojan::TrojanUdpFlowIo;
        while let Ok(packet) = flow_io.read_packet(&mut recv_stream).await {
            if recv_tx.send(packet).is_err() {
                break;
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
