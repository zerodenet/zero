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
    let mut handler = RelayPacketHandler {
        proxy: request.proxy,
        dispatch: request.dispatch,
        relay: request.relay,
        inbound_tag: request.inbound_tag,
        pending_control_traffic: request.pending_control_traffic,
    };
    request
        .relay_session
        .handle_packet(sender, request.payload, &mut handler)
        .await
}

struct RelayPacketHandler<'a> {
    proxy: &'a Proxy,
    dispatch: &'a mut UdpDispatch,
    relay: &'a TokioDatagramSocket,
    inbound_tag: &'a str,
    pending_control_traffic: &'a mut StreamTraffic,
}

impl socks5::udp::Socks5InboundUdpRelayPacketHandler for RelayPacketHandler<'_> {
    type Error = EngineError;

    async fn handle_client_packet(&mut self, payload: &[u8]) -> Result<(), Self::Error> {
        if let Err(error) = dispatch::dispatch_packet(
            self.proxy,
            payload,
            self.dispatch,
            self.pending_control_traffic,
        )
        .await
        {
            warn!(
                inbound_tag = self.inbound_tag,
                protocol = "socks5_udp",
                error = %error,
                "failed to process UDP packet"
            );
        }

        Ok(())
    }

    async fn handle_peer_response(
        &mut self,
        client: zero_traits::SocketAddress,
        sender: zero_traits::SocketAddress,
        payload: &[u8],
    ) -> Result<(), Self::Error> {
        direct_response::forward_relay_socket_response(
            self.proxy,
            self.dispatch,
            self.relay,
            zero_platform_tokio::socket_address_to_socket_addr(client),
            zero_platform_tokio::socket_address_to_socket_addr(sender),
            payload,
        )
        .await
    }

    async fn handle_unexpected_sender(
        &mut self,
        sender: zero_traits::SocketAddress,
    ) -> Result<(), Self::Error> {
        debug!(?sender, "dropping udp packet from unexpected sender");
        Ok(())
    }
}
