//! Shared UDP helper functions and types used by inbound handlers.
//!
//! Moved from outbound/direct.rs — these are runtime orchestration, not outbound protocol logic.

use std::net::SocketAddr;

use zero_core::{Address, Session};
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::Proxy;

/// A normalized response from any chain outbound (SS, H2, Trojan, Mieru).
///
/// Used as the item type in per-dispatcher response channels so inbound
/// handlers can `select!` on responses from all chain outbounds.
#[derive(Debug)]
pub struct UdpChainResponse {
    pub target: Address,
    pub port: u16,
    pub payload: Vec<u8>,
}

/// Resolve target address for direct UDP outbound.
#[allow(dead_code)]
pub(crate) async fn resolve_udp_target(
    proxy: &Proxy,
    session: &Session,
) -> Result<SocketAddr, EngineError> {
    proxy
        .protocols
        .direct_outbound
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
