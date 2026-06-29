use std::net::SocketAddr;

use tracing::{debug, warn};
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use super::{direct_response, dispatch};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::Proxy;
use crate::transport::StreamTraffic;

pub(super) struct RelayPacketRequest<'a> {
    pub proxy: &'a Proxy,
    pub dispatch: &'a mut UdpDispatch,
    pub relay: &'a TokioDatagramSocket,
    pub inbound_tag: &'a str,
    pub pending_control_traffic: &'a mut StreamTraffic,
    pub relay_session: &'a mut socks5::udp::Socks5InboundUdpRelaySession,
    pub sender: SocketAddr,
    pub payload: &'a [u8],
}

pub(super) async fn handle_relay_packet(
    request: RelayPacketRequest<'_>,
) -> Result<(), EngineError> {
    let sender = zero_platform_tokio::socket_addr_to_socket_address(request.sender);
    match request
        .relay_session
        .classify_packet(sender, request.payload)
    {
        socks5::udp::Socks5InboundUdpRelayPacketAction::ClientPacket { payload } => {
            if let Err(error) = dispatch::dispatch_packet(
                request.proxy,
                payload,
                request.dispatch,
                request.pending_control_traffic,
            )
            .await
            {
                warn!(
                    inbound_tag = request.inbound_tag,
                    protocol = "socks5_udp",
                    error = %error,
                    "failed to process UDP packet"
                );
            }
        }
        socks5::udp::Socks5InboundUdpRelayPacketAction::PeerResponse {
            client,
            sender,
            payload,
        } => {
            direct_response::forward_relay_socket_response(
                request.proxy,
                request.dispatch,
                request.relay,
                zero_platform_tokio::socket_address_to_socket_addr(client),
                zero_platform_tokio::socket_address_to_socket_addr(sender),
                payload,
            )
            .await?;
        }
        socks5::udp::Socks5InboundUdpRelayPacketAction::UnexpectedSender { sender } => {
            debug!(?sender, "dropping udp packet from unexpected sender");
        }
    }

    Ok(())
}
