use std::net::SocketAddr;

use tracing::warn;
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use super::protocol_glue;
use crate::inbound::udp_response::write_direct_response;
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
    write_socks5_direct_response(relay, client_addr, &response).await?;

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

    match write_socks5_direct_response(relay, client_addr, &response).await {
        Ok(_) => {}
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

async fn write_socks5_direct_response(
    relay: &TokioDatagramSocket,
    client_addr: SocketAddr,
    response: &UdpDirectResponseParts<'_, '_>,
) -> Result<usize, EngineError> {
    write_direct_response(response, || async {
        protocol_glue::send_client_response_for_target(
            relay,
            client_addr,
            &response.target,
            response.port,
            response.payload,
        )
        .await
    })
    .await
}
