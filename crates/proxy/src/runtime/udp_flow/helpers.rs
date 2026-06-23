use std::net::SocketAddr;

use tokio::time::{sleep_until, Instant as TokioInstant};
use zero_core::Address;

use crate::logging::log_session_finished;

use super::sessions::CompletedUdpFlow;

pub(crate) fn log_completed_udp_flow(completed: CompletedUdpFlow) {
    log_session_finished(
        &completed.record,
        completed
            .upstream
            .as_ref()
            .map(|(server, port)| (server.as_str(), *port)),
    );
}

pub(crate) async fn wait_for_upstream_idle(deadline: Option<TokioInstant>) {
    match deadline {
        Some(deadline) => sleep_until(deadline).await,
        None => std::future::pending::<()>().await,
    }
}

pub(crate) fn address_from_socket_addr(addr: SocketAddr) -> Address {
    match zero_platform_tokio::socket_addr_to_ip(addr) {
        zero_traits::IpAddress::V4(ip) => Address::Ipv4(ip),
        zero_traits::IpAddress::V6(ip) => Address::Ipv6(ip),
    }
}
