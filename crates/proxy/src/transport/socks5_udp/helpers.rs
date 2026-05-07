use std::net::{IpAddr, SocketAddr};

use tokio::time::{sleep_until, Instant as TokioInstant};
use zero_core::Address;
use zero_engine::EngineError;

use super::super::super::logging::log_session_finished;
use super::super::udp_sessions::CompletedUdpFlow;
use super::super::upstream_socks5_udp::ActiveUpstreamSocks5UdpAssociation;

pub(super) fn log_completed_udp_flow(completed: CompletedUdpFlow) {
    log_session_finished(
        &completed.record,
        completed
            .upstream
            .as_ref()
            .map(|(server, port)| (server.as_str(), *port)),
    );
}

pub(super) async fn recv_upstream_packet(
    association: Option<&ActiveUpstreamSocks5UdpAssociation>,
    buf: &mut [u8],
) -> Result<usize, EngineError> {
    match association {
        Some(association) => association.recv_packet(buf).await,
        None => std::future::pending::<Result<usize, EngineError>>().await,
    }
}

pub(super) async fn wait_for_upstream_idle(deadline: Option<TokioInstant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

pub(super) fn address_from_socket_addr(addr: SocketAddr) -> Address {
    match addr.ip() {
        IpAddr::V4(ip) => Address::Ipv4(ip.octets()),
        IpAddr::V6(ip) => Address::Ipv6(ip.octets()),
    }
}
