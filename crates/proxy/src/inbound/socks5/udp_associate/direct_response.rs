use std::net::SocketAddr;

use tracing::warn;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{
    record_direct_udp_response_received, udp_response_target_from_socket_addr,
};
use crate::runtime::Proxy;

pub(super) async fn forward_relay_socket_response(
    proxy: &Proxy,
    dispatch: &UdpDispatch,
    relay: &TokioDatagramSocket,
    client_addr: SocketAddr,
    sender: SocketAddr,
    payload: &[u8],
) -> Result<(), EngineError> {
    let response_accounting =
        record_direct_udp_response_received(proxy, dispatch, sender, payload.len());
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

    let response_accounting =
        record_direct_udp_response_received(proxy, dispatch, sender, payload.len());

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
    let (target, port) = udp_response_target_from_socket_addr(sender);
    let response = socks5::udp::Socks5UdpClientResponse::new(&target, port, payload);
    udp_session
        .send_client_response(
            relay,
            zero_platform_tokio::socket_addr_to_socket_address(client_addr),
            response,
        )
        .await
        .map_err(|error| error.into_mapped(EngineError::from))
}
