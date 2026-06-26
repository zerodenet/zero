//! Shadowsocks UDP socket flow transport helpers.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::{debug, warn};
use zero_core::{Address, UdpFlowPacket};
use zero_engine::EngineError;

pub type ShadowsocksUdpResponse = (Address, u16, Vec<u8>);

pub struct ShadowsocksUdpSocketFlow {
    socket: Arc<tokio::net::UdpSocket>,
    endpoint: SocketAddr,
    resume: shadowsocks::ShadowsocksUdpFlowResume,
    recv_tx: broadcast::Sender<ShadowsocksUdpResponse>,
}

pub async fn establish_shadowsocks_udp_socket_flow(
    endpoint: SocketAddr,
    resume: shadowsocks::ShadowsocksUdpFlowResume,
) -> Result<ShadowsocksUdpSocketFlow, EngineError> {
    let socket = Arc::new(bind_for_endpoint(endpoint).await?);
    let (recv_tx, _) = broadcast::channel::<ShadowsocksUdpResponse>(32);
    spawn_recv_loop(socket.clone(), resume.clone(), recv_tx.clone());

    Ok(ShadowsocksUdpSocketFlow {
        socket,
        endpoint,
        resume,
        recv_tx,
    })
}

impl ShadowsocksUdpSocketFlow {
    pub fn subscribe(&self) -> broadcast::Receiver<ShadowsocksUdpResponse> {
        self.recv_tx.subscribe()
    }

    pub async fn send_packet(&self, packet: UdpFlowPacket) -> Result<(), EngineError> {
        let packet = shadowsocks::udp_flow_packet(&packet.target, packet.port, &packet.payload);
        let datagram = packet.encode_with(&self.resume)?;
        self.socket.send_to(&datagram, self.endpoint).await?;
        Ok(())
    }
}

async fn bind_for_endpoint(endpoint: SocketAddr) -> Result<tokio::net::UdpSocket, std::io::Error> {
    let bind_addr = match endpoint {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    tokio::net::UdpSocket::bind(bind_addr).await
}

fn spawn_recv_loop(
    socket: Arc<tokio::net::UdpSocket>,
    resume: shadowsocks::ShadowsocksUdpFlowResume,
    recv_tx: broadcast::Sender<ShadowsocksUdpResponse>,
) {
    tokio::spawn(recv_loop(socket, resume, recv_tx));
}

async fn recv_loop(
    socket: Arc<tokio::net::UdpSocket>,
    resume: shadowsocks::ShadowsocksUdpFlowResume,
    recv_tx: broadcast::Sender<ShadowsocksUdpResponse>,
) {
    let mut buf = vec![0u8; 4096];
    loop {
        let (n, sender) = match socket.recv_from(&mut buf).await {
            Ok(r) => r,
            Err(error) => {
                warn!(error = %error, "shadowsocks udp recv loop stopped");
                break;
            }
        };
        let datagram = &buf[..n];
        let Some(packet) = resume.decode_flow_packet(datagram) else {
            warn!(
                upstream = %sender,
                bytes = n,
                "failed to decode shadowsocks udp response"
            );
            continue;
        };
        let (target, port, payload) = packet.into_parts();
        debug!(
            upstream = %sender,
            target = ?target,
            port = port,
            bytes = payload.len(),
            "decoded shadowsocks udp response"
        );
        if recv_tx.send((target, port, payload)).is_err() {
            break;
        }
    }
}
