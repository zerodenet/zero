use std::net::SocketAddr;

use tracing::{debug, warn};
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;

use super::{direct_response, dispatch};
use crate::runtime::udp_association::UdpAssociationHandler;
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_flow::helpers::{UdpChainResponseParts, UdpUpstreamResponseParts};
use crate::runtime::Proxy;
use crate::transport::StreamTraffic;

pub(super) struct Socks5UdpAssociationHandler {
    association: socks5::udp::Socks5InboundUdpAssociationSession,
}

struct Socks5UdpRelayPacketBridge<'a> {
    proxy: &'a Proxy,
    dispatch: &'a mut UdpDispatch,
    relay: &'a TokioDatagramSocket,
    association: socks5::udp::Socks5InboundUdpAssociationSession,
    pending_control_traffic: &'a mut StreamTraffic,
    inbound_tag: String,
}

impl Socks5UdpAssociationHandler {
    pub(super) fn new(request: socks5::udp::Socks5UdpAssociateRequest) -> Self {
        Self {
            association: socks5::Socks5Inbound.accept_udp_association(request),
        }
    }
}

impl UdpAssociationHandler for Socks5UdpAssociationHandler {
    async fn handle_client_datagram(
        &mut self,
        proxy: &Proxy,
        dispatch: &mut UdpDispatch,
        relay: &TokioDatagramSocket,
        pending_control_traffic: &mut StreamTraffic,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let inbound_tag = dispatch.inbound_tag().to_owned();
        let sender = zero_platform_tokio::socket_addr_to_socket_address(sender);
        let mut bridge = Socks5UdpRelayPacketBridge {
            proxy,
            dispatch,
            relay,
            association: self.association,
            pending_control_traffic,
            inbound_tag,
        };
        self.association
            .dispatch_relay_packet(sender, payload, &mut bridge)
            .await
    }

    async fn write_direct_response(
        &mut self,
        proxy: &Proxy,
        dispatch: &UdpDispatch,
        relay: &TokioDatagramSocket,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        direct_response::forward_relay_socket_response(
            proxy,
            dispatch,
            &self.association,
            relay,
            sender,
            payload,
        )
        .await
    }

    async fn write_upstream_response(
        &mut self,
        relay: &TokioDatagramSocket,
        response: &UdpUpstreamResponseParts<'_>,
    ) -> Result<usize, EngineError> {
        self.write_client_response(relay, &response.target, response.port, &response.payload)
            .await
    }

    async fn write_chain_response(
        &mut self,
        relay: &TokioDatagramSocket,
        response: &UdpChainResponseParts<'_>,
    ) -> Result<usize, EngineError> {
        self.write_client_response(relay, &response.target, response.port, &response.payload)
            .await
    }
}

impl Socks5UdpAssociationHandler {
    async fn write_client_response(
        &self,
        relay: &TokioDatagramSocket,
        target: &zero_core::Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, EngineError> {
        self.association
            .send_current_client_response_for_target(relay, target, port, payload)
            .await
            .map_err(|error| error.into_mapped(EngineError::from))
    }
}

impl socks5::udp::Socks5InboundUdpRelayPacketDispatcher for Socks5UdpRelayPacketBridge<'_> {
    type Error = EngineError;

    async fn dispatch_client_packet(&mut self, payload: &[u8]) -> Result<(), Self::Error> {
        if let Err(error) = dispatch::dispatch_packet(
            self.proxy,
            &self.association,
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

    async fn dispatch_peer_response(
        &mut self,
        sender: zero_traits::SocketAddress,
        payload: &[u8],
    ) -> Result<(), Self::Error> {
        direct_response::forward_relay_peer_response(
            self.proxy,
            self.dispatch,
            &self.association,
            self.relay,
            sender,
            payload,
        )
        .await
    }

    async fn dispatch_unexpected_sender(
        &mut self,
        sender: zero_traits::SocketAddress,
    ) -> Result<(), Self::Error> {
        debug!(?sender, "dropping udp packet from unexpected sender");
        Ok(())
    }
}
