//! Shared UDP helper functions and types used by inbound handlers.
//!
//! These helpers are runtime orchestration, not outbound protocol logic.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::Proxy;

/// Resolve target address for direct UDP outbound.
#[allow(dead_code)]
pub(crate) async fn resolve_udp_target(
    proxy: &Proxy,
    session: &Session,
) -> Result<SocketAddr, EngineError> {
    proxy
        .protocols
        .direct_connector()
        .resolve_target_addr(session, proxy.resolver.as_ref())
        .await
        .map_err(Into::into)
}

/// Send UDP packet directly to target.
#[allow(dead_code)]
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

pub(crate) fn datagram_bind_addr_for_peer(peer: SocketAddr) -> SocketAddr {
    match peer {
        SocketAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
        SocketAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
    }
}

pub(crate) async fn bind_datagram_socket_for_peer(
    peer: SocketAddr,
) -> Result<TokioDatagramSocket, EngineError> {
    TokioDatagramSocket::bind_addr(datagram_bind_addr_for_peer(peer))
        .await
        .map_err(Into::into)
}

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
