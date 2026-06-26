//! Trojan transport helpers.

use std::io;
use std::path::Path;

use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{broadcast, mpsc};
use zero_config::ClientTlsConfig;
use zero_core::{Session, UdpFlowPacket};
use zero_engine::EngineError;
use zero_platform_tokio::{TcpRelayStream, TokioSocket};
use zero_traits::AsyncSocket;

pub struct TrojanUdpTlsOptions<'a> {
    pub profile: trojan::TrojanUdpTlsProfile,
    pub source_dir: Option<&'a Path>,
    pub server: &'a str,
}

pub async fn open_trojan_udp_tls_stream(
    socket: TokioSocket,
    options: TrojanUdpTlsOptions<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let tls_config = tls_config(options.profile);
    crate::tls::connect_tls_upstream(socket, &tls_config, options.source_dir, options.server).await
}

pub async fn open_trojan_udp_tls_relay_stream(
    stream: TcpRelayStream,
    options: TrojanUdpTlsOptions<'_>,
) -> Result<TcpRelayStream, EngineError> {
    let tls_config = tls_config(options.profile);
    crate::tls::connect_tls_stream(stream, &tls_config, options.source_dir, options.server).await
}

pub struct TrojanUdpFlowStream {
    pub send_tx: mpsc::Sender<UdpFlowPacket>,
    pub recv_tx: broadcast::Sender<trojan::TrojanUdpPacket>,
}

pub struct TrojanUdpFlowStreamRequest<'a> {
    pub stream: TcpRelayStream,
    pub session: &'a Session,
    pub resume: &'a trojan::TrojanUdpFlowResume,
}

pub async fn establish_trojan_udp_flow_stream(
    request: TrojanUdpFlowStreamRequest<'_>,
) -> Result<TrojanUdpFlowStream, EngineError> {
    let mut stream = request.stream;
    let flow_io = trojan::TrojanUdpFlowIo;
    flow_io
        .establish_with_resume(&mut stream, request.session, request.resume)
        .await?;

    let (read_half, write_half) = tokio::io::split(stream);
    let (send_tx, send_rx) = mpsc::channel::<UdpFlowPacket>(32);
    let (recv_tx, _) = broadcast::channel::<trojan::TrojanUdpPacket>(32);

    spawn_trojan_udp_send_task(send_rx, WriteOnlySocket(write_half));
    spawn_trojan_udp_recv_task(ReadOnlySocket(read_half), recv_tx.clone());

    Ok(TrojanUdpFlowStream { send_tx, recv_tx })
}

fn spawn_trojan_udp_send_task(
    mut send_rx: mpsc::Receiver<UdpFlowPacket>,
    mut send_stream: WriteOnlySocket,
) {
    tokio::spawn(async move {
        let flow_io = trojan::TrojanUdpFlowIo;
        while let Some(packet) = send_rx.recv().await {
            let packet = trojan::udp_flow_packet(&packet.target, packet.port, &packet.payload);
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

fn spawn_trojan_udp_recv_task(
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

fn tls_config(tls_profile: trojan::TrojanUdpTlsProfile) -> ClientTlsConfig {
    ClientTlsConfig {
        server_name: tls_profile.server_name().map(|s| s.to_owned()),
        disable_sni: false,
        ca_cert_path: None,
        insecure: tls_profile.insecure(),
        alpn: Vec::new(),
        client_fingerprint: tls_profile.client_fingerprint().map(|s| s.to_owned()),
    }
}
