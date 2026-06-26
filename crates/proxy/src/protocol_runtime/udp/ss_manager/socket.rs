use super::bridge::BridgeWaiters;
use super::codec;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use tracing::{debug, warn};
use zero_core::{Address, Error};
use zero_traits::DatagramCodec;

pub(super) fn bind_for_target(target_addr: SocketAddr) -> Arc<tokio::net::UdpSocket> {
    let bind_addr = match target_addr {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    };
    Arc::new({
        let socket = std::net::UdpSocket::bind(bind_addr).expect("ss: bind");
        socket.set_nonblocking(true).expect("ss: nonblocking");
        tokio::net::UdpSocket::from_std(socket).expect("ss: tokio")
    })
}

pub(super) fn spawn_recv_loop(
    socket: Arc<tokio::net::UdpSocket>,
    codec: Arc<dyn DatagramCodec<Address, Error = Error>>,
    waiters: BridgeWaiters,
) {
    tokio::spawn(recv_loop(socket, codec, waiters));
}

async fn recv_loop(
    socket: Arc<tokio::net::UdpSocket>,
    codec: Arc<dyn DatagramCodec<Address, Error = Error>>,
    waiters: BridgeWaiters,
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
        let packet = &buf[..n];
        let Ok((target, port, payload)) = codec::decode_packet(codec.as_ref(), packet) else {
            warn!(
                upstream = %sender,
                bytes = n,
                "failed to decode shadowsocks udp response"
            );
            continue;
        };
        debug!(
            upstream = %sender,
            target = ?target,
            port = port,
            bytes = payload.len(),
            "decoded shadowsocks udp response"
        );
        if !waiters.deliver(target.clone(), port, payload) {
            warn!(
                target = ?target,
                port = port,
                "no waiter for shadowsocks udp response"
            );
        }
    }
}
