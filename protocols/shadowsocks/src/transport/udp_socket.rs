use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::{debug, warn};
use zero_core::{Address, UdpFlowPacket};
use zero_traits::DatagramCodec;
use zero_transport::managed_udp::ManagedDatagramConnectionOps;
use zero_transport::RuntimeError;

use super::{ShadowsocksManagedDatagramFlowResume, ShadowsocksUdpResponse};

pub struct ShadowsocksUdpSocketFlow {
    socket: Arc<tokio::net::UdpSocket>,
    endpoint: SocketAddr,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    recv_tx: broadcast::Sender<ShadowsocksUdpResponse>,
}

pub fn managed_socket_flow_from_resume(
    resume: &ShadowsocksManagedDatagramFlowResume,
) -> crate::udp::ShadowsocksUdpSocketFlowSpec {
    resume.socket_flow_spec()
}

pub async fn establish_shadowsocks_udp_socket_flow(
    endpoint: SocketAddr,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
) -> Result<ShadowsocksUdpSocketFlow, RuntimeError> {
    let socket = Arc::new(bind_for_endpoint(endpoint).await?);
    let (recv_tx, _) = broadcast::channel::<ShadowsocksUdpResponse>(32);
    spawn_recv_loop(socket.clone(), codec.clone(), recv_tx.clone());
    Ok(ShadowsocksUdpSocketFlow {
        socket,
        endpoint,
        codec,
        recv_tx,
    })
}

pub async fn establish_shadowsocks_udp_socket_flow_with_resume(
    endpoint: SocketAddr,
    resume: ShadowsocksManagedDatagramFlowResume,
) -> Result<ShadowsocksUdpSocketFlow, RuntimeError> {
    establish_shadowsocks_udp_socket_flow(endpoint, resume.into_shared_managed_socket_flow_codec())
        .await
}

impl ShadowsocksUdpSocketFlow {
    pub fn subscribe(&self) -> broadcast::Receiver<ShadowsocksUdpResponse> {
        self.recv_tx.subscribe()
    }

    pub async fn send_packet(&self, packet: UdpFlowPacket) -> Result<(), RuntimeError> {
        self.send_datagram(&packet.target, packet.port, &packet.payload)
            .await
    }

    pub async fn send_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), RuntimeError> {
        let datagram = self.codec.encode(target, port, payload)?;
        self.socket.send_to(&datagram, self.endpoint).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ManagedDatagramConnectionOps for ShadowsocksUdpSocketFlow {
    type SendError = RuntimeError;

    async fn send_protocol_datagram(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<(), Self::SendError> {
        self.send_datagram(target, port, payload).await
    }

    fn subscribe_protocol_datagrams(&self) -> broadcast::Receiver<ShadowsocksUdpResponse> {
        self.subscribe()
    }

    fn closed_message_for_datagram_connection(&self) -> &'static str {
        "ss upstream closed"
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
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    recv_tx: broadcast::Sender<ShadowsocksUdpResponse>,
) {
    tokio::spawn(recv_loop(socket, codec, recv_tx));
}

async fn recv_loop(
    socket: Arc<tokio::net::UdpSocket>,
    codec: Arc<dyn DatagramCodec<Address, Error = zero_core::Error>>,
    recv_tx: broadcast::Sender<ShadowsocksUdpResponse>,
) {
    let mut buf = vec![0u8; 4096];
    loop {
        let (n, sender) = match socket.recv_from(&mut buf).await {
            Ok(result) => result,
            Err(error) => {
                warn!(error = %error, "shadowsocks udp recv loop stopped");
                break;
            }
        };
        let datagram = &buf[..n];
        let Some((target, port, payload)) = codec.decode(datagram) else {
            warn!(upstream = %sender, bytes = n, "failed to decode shadowsocks udp response");
            continue;
        };
        debug!(
            upstream = %sender,
            target = ?target,
            port,
            bytes = payload.len(),
            "decoded shadowsocks udp response"
        );
        if recv_tx.send((target, port, payload)).is_err() {
            break;
        }
    }
}
