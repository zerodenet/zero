//! Direct outbound implementation

use std::net::SocketAddr;

use zero_core::Session;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::Proxy;

/// Resolve target address for direct UDP outbound
pub async fn resolve_udp_target(
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

/// Send UDP packet directly to target
#[allow(dead_code)]
pub async fn send_direct_udp_packet(
    socket: &TokioDatagramSocket,
    target_addr: SocketAddr,
    payload: &[u8],
) -> Result<usize, EngineError> {
    socket
        .send_to_addr(payload, target_addr)
        .await
        .map_err(Into::into)
}
