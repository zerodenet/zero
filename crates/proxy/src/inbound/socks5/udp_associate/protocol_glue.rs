use std::net::SocketAddr;

use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::Proxy;

pub(super) type RelaySession = socks5::udp::Socks5InboundUdpRelaySession;

pub(super) fn new_relay_session() -> RelaySession {
    socks5::udp::Socks5InboundUdpRelaySession::new()
}

pub(super) async fn decode_dispatch(
    proxy: &Proxy,
    packet: &[u8],
) -> Result<Option<socks5::udp::Socks5InboundUdpDispatchView>, EngineError> {
    let udp_responder = socks5::Socks5Inbound.udp_responder();
    let Some(request) = udp_responder
        .decode_dispatch_parts_or_resolve_local_dns(packet, proxy.resolver.as_ref())
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(request))
}

pub(super) async fn send_client_response_for_target(
    relay: &TokioDatagramSocket,
    client_addr: SocketAddr,
    target: &zero_core::Address,
    port: u16,
    payload: &[u8],
) -> Result<usize, EngineError> {
    let udp_responder = socks5::Socks5Inbound.udp_responder();
    udp_responder
        .send_client_response_for_target(
            relay,
            zero_platform_tokio::socket_addr_to_socket_address(client_addr),
            target,
            port,
            payload,
        )
        .await
        .map_err(|error| error.into_mapped(EngineError::from))
}
