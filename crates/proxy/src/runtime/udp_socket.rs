//! Neutral UDP endpoint resolution, socket binding, and packet sending.

#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use std::net::SocketAddr;
#[cfg(feature = "socks5")]
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[cfg(feature = "socks5")]
use zero_core::Address;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use zero_engine::EngineError;
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
use zero_platform_tokio::TokioDatagramSocket;

#[cfg(feature = "socks5")]
use crate::runtime::Proxy;

/// Send UDP packet directly to target.
#[cfg(any(
    feature = "socks5",
    feature = "vless",
    feature = "hysteria2",
    feature = "shadowsocks",
    feature = "trojan",
    feature = "vmess",
    feature = "mieru"
))]
pub(crate) async fn send_direct_udp_packet(
    socket: &TokioDatagramSocket,
    target_addr: SocketAddr,
    payload: &[u8],
) -> Result<usize, EngineError> {
    socket
        .send_to_addr(payload, target_addr)
        .await
        .map_err(Into::into)
}

#[cfg(feature = "socks5")]
pub(crate) fn datagram_bind_addr_for_peer(peer: SocketAddr) -> SocketAddr {
    match peer {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    }
}

#[cfg(feature = "socks5")]
pub(crate) async fn bind_datagram_socket_for_peer(
    peer: SocketAddr,
) -> Result<TokioDatagramSocket, EngineError> {
    TokioDatagramSocket::bind_addr(datagram_bind_addr_for_peer(peer))
        .await
        .map_err(Into::into)
}

#[cfg(feature = "socks5")]
pub(crate) async fn resolve_udp_peer_endpoint(
    proxy: &Proxy,
    address: &Address,
    port: u16,
    error_message: &'static str,
) -> Result<(SocketAddr, TokioDatagramSocket), EngineError> {
    let endpoint = proxy
        .protocols
        .direct_connector()
        .resolve_address(address, port, proxy.resolver.as_ref(), error_message)
        .await
        .map_err(EngineError::from)?;
    let socket = bind_datagram_socket_for_peer(endpoint).await?;
    Ok((endpoint, socket))
}
