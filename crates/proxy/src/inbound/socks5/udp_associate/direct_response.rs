use std::net::SocketAddr;

use tracing::warn;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::UdpInboundResponseAccounting;
use crate::runtime::Proxy;

pub(super) async fn forward_relay_socket_response(
    proxy: &Proxy,
    dispatch: &UdpDispatch,
    relay: &TokioDatagramSocket,
    client_addr: SocketAddr,
    sender: SocketAddr,
    payload: &[u8],
) -> Result<(), EngineError> {
    let session_id = dispatch.direct_response_session_id(sender);
    let response_accounting =
        UdpInboundResponseAccounting::record_received(proxy, session_id, payload.len());
    let sent = forward_direct_udp_response(relay, client_addr, sender, payload).await?;
    response_accounting.record_sent(sent);

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

    let session_id = dispatch.direct_response_session_id(sender);
    let response_accounting =
        UdpInboundResponseAccounting::record_received(proxy, session_id, payload.len());

    match forward_direct_udp_response(relay, client_addr, sender, payload).await {
        Ok(sent) => {
            response_accounting.record_sent(sent);
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
    udp_session
        .send_response_to_client_socket_addr(
            relay,
            zero_platform_tokio::socket_addr_to_socket_address(client_addr),
            zero_platform_tokio::socket_addr_to_socket_address(sender),
            payload,
        )
        .await
        .map_err(|error| error.into_mapped(EngineError::from))
}
