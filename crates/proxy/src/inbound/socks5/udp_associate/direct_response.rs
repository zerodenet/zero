use std::net::SocketAddr;

use tracing::warn;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;

pub(super) async fn forward_relay_socket_response(
    proxy: &Proxy,
    dispatch: &UdpDispatch,
    relay: &TokioDatagramSocket,
    client_addr: SocketAddr,
    sender: SocketAddr,
    payload: &[u8],
) -> Result<(), EngineError> {
    if let Some(session_id) = dispatch.direct_response_session_id(sender) {
        proxy.record_session_outbound_rx(session_id, payload.len() as u64);
        let sent = forward_direct_udp_response(relay, client_addr, sender, payload).await?;
        proxy.record_session_inbound_tx(session_id, sent as u64);
    } else {
        forward_direct_udp_response(relay, client_addr, sender, payload).await?;
    }

    Ok(())
}

pub(super) async fn forward_dispatch_socket_response(
    proxy: &Proxy,
    dispatch: &UdpDispatch,
    relay: &TokioDatagramSocket,
    client_addr: Option<SocketAddr>,
    inbound_tag: &str,
    sender: SocketAddr,
    payload: &[u8],
) {
    let Some(client_addr) = client_addr else {
        return;
    };

    if let Some(session_id) = dispatch.direct_response_session_id(sender) {
        proxy.record_session_outbound_rx(session_id, payload.len() as u64);
    }

    match forward_direct_udp_response(relay, client_addr, sender, payload).await {
        Ok(sent) => {
            if let Some(session_id) = dispatch.direct_response_session_id(sender) {
                proxy.record_session_inbound_tx(session_id, sent as u64);
            }
        }
        Err(error) => {
            warn!(
                inbound_tag = inbound_tag,
                protocol = "socks5_udp",
                error = %error,
                "failed to forward direct UDP response"
            );
        }
    }
}

pub(super) async fn forward_direct_udp_response(
    relay: &TokioDatagramSocket,
    client_addr: SocketAddr,
    sender: SocketAddr,
    payload: &[u8],
) -> Result<usize, EngineError> {
    let udp_session = socks5::Socks5Inbound.udp_session();
    let upstream_address = crate::runtime::udp_flow::helpers::address_from_socket_addr(sender);
    udp_session
        .send_response_to_client(
            relay,
            zero_platform_tokio::socket_addr_to_ip(client_addr),
            client_addr.port(),
            &upstream_address,
            sender.port(),
            payload,
        )
        .await
        .map_err(socks5_udp_relay_error_to_engine)
}

fn socks5_udp_relay_error_to_engine(
    error: socks5::udp::Socks5UdpRelayError<std::io::Error>,
) -> EngineError {
    match error {
        socks5::udp::Socks5UdpRelayError::Socket(error) => EngineError::from(error),
        socks5::udp::Socks5UdpRelayError::Protocol(error) => EngineError::from(error),
    }
}
