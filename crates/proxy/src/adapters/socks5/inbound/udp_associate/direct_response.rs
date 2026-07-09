use std::net::SocketAddr;

use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use crate::runtime::udp_dispatch::UdpDispatch;

use crate::runtime::udp_flow::helpers::record_direct_udp_response_parts;
use crate::runtime::udp_response::write_direct_response;
use crate::runtime::Proxy;

pub(super) async fn forward_relay_socket_response(
    proxy: &Proxy,
    dispatch: &UdpDispatch,
    association: &socks5::udp::Socks5InboundUdpAssociationSession,
    relay: &TokioDatagramSocket,
    sender: SocketAddr,
    payload: &[u8],
) -> Result<(), EngineError> {
    let response = record_direct_udp_response_parts(proxy, dispatch, sender, payload);
    write_direct_response(&response, || async {
        association
            .send_current_client_response_for_target(
                relay,
                &response.target,
                response.port,
                response.payload,
            )
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    })
    .await?;

    Ok(())
}

pub(super) async fn forward_relay_peer_response(
    proxy: &Proxy,
    dispatch: &UdpDispatch,
    association: &socks5::udp::Socks5InboundUdpAssociationSession,
    relay: &TokioDatagramSocket,
    sender: zero_traits::SocketAddress,
    payload: &[u8],
) -> Result<(), EngineError> {
    let sender_socket_addr = zero_platform_tokio::socket_address_to_socket_addr(sender);
    let response_parts =
        record_direct_udp_response_parts(proxy, dispatch, sender_socket_addr, payload);
    write_direct_response(&response_parts, || async {
        association
            .send_current_client_peer_response_parts(relay, sender, payload)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    })
    .await?;

    Ok(())
}
