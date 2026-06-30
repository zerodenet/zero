use std::net::SocketAddr;

use tracing::warn;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{record_direct_udp_response_parts, UdpDirectResponseParts};
use crate::runtime::Proxy;

pub(super) async fn forward_relay_socket_response(
    proxy: &Proxy,
    dispatch: &UdpDispatch,
    relay: &TokioDatagramSocket,
    client_addr: SocketAddr,
    sender: SocketAddr,
    payload: &[u8],
) -> Result<(), EngineError> {
    let response = record_direct_udp_response_parts(proxy, dispatch, sender, payload);
    let sent = forward_direct_udp_response(relay, client_addr, &response).await?;
    response.accounting.record_sent(sent);

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

    let response = record_direct_udp_response_parts(proxy, dispatch, sender, payload);

    match forward_direct_udp_response(relay, client_addr, &response).await {
        Ok(sent) => {
            response.accounting.record_sent(sent);
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
    response: &UdpDirectResponseParts<'_, '_>,
) -> Result<usize, EngineError> {
    let udp_session = socks5::Socks5Inbound.udp_session();
    udp_session
        .send_client_response_for_target(
            relay,
            zero_platform_tokio::socket_addr_to_socket_address(client_addr),
            &response.target,
            response.port,
            response.payload,
        )
        .await
        .map_err(|error| error.into_mapped(EngineError::from))
}
