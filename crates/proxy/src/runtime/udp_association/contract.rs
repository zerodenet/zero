use std::net::SocketAddr;
use std::vec::Vec;

use zero_core::{
    InboundUdpAssociation, InboundUdpAssociationDispatcher, InboundUdpAssociationResponder,
    InboundUdpAssociationResponse, InboundUdpDispatch,
};
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;
use zero_traits::DnsResolver;

use crate::protocol_registry::UdpRuntimeServices;
use crate::runtime::udp_delivery::write_direct_response as write_direct_udp_response;
use crate::runtime::udp_delivery::{
    record_direct_udp_response_parts, UdpChainResponseParts, UdpUpstreamResponseParts,
};
use crate::runtime::udp_dispatch::UdpDispatch;
use crate::runtime::udp_ingress::dispatch_inbound_udp_packet;
use crate::runtime::Proxy;
use crate::transport::{MeteredStream, StreamTraffic};

enum UdpAssociationDispatchOutcome {
    ClientHandled,
    PeerResponse {
        sender: zero_traits::SocketAddress,
        payload: Vec<u8>,
    },
    UnexpectedSender {
        sender: zero_traits::SocketAddress,
    },
}

struct UdpAssociationDispatchBridge<'a> {
    proxy: &'a Proxy,
    services: UdpRuntimeServices,
    dispatch: &'a mut UdpDispatch,
    pending_control_traffic: &'a mut StreamTraffic,
    outcome: UdpAssociationDispatchOutcome,
}

impl InboundUdpAssociationDispatcher for UdpAssociationDispatchBridge<'_> {
    type Error = EngineError;

    async fn dispatch_local_dns(&mut self, domain: &str) -> Result<(), Self::Error> {
        let _ = self.proxy.resolver.resolve(domain).await;
        Ok(())
    }

    async fn dispatch_inbound_packet(
        &mut self,
        inbound_dispatch: InboundUdpDispatch,
        protocol_overhead_bytes: u64,
    ) -> Result<(), Self::Error> {
        let session_id =
            dispatch_inbound_udp_packet(self.proxy, self.dispatch, &inbound_dispatch, None).await?;

        self.services
            .record_session_inbound_traffic(session_id, *self.pending_control_traffic);
        *self.pending_control_traffic = StreamTraffic::default();
        self.services
            .record_session_inbound_rx(session_id, protocol_overhead_bytes);

        Ok(())
    }

    async fn dispatch_peer_response(
        &mut self,
        sender: zero_traits::SocketAddress,
        payload: &[u8],
    ) -> Result<(), Self::Error> {
        self.outcome = UdpAssociationDispatchOutcome::PeerResponse {
            sender,
            payload: payload.to_vec(),
        };
        Ok(())
    }

    async fn dispatch_unexpected_sender(
        &mut self,
        sender: zero_traits::SocketAddress,
    ) -> Result<(), Self::Error> {
        self.outcome = UdpAssociationDispatchOutcome::UnexpectedSender { sender };
        Ok(())
    }
}

pub(crate) trait UdpAssociationHandler {
    async fn handle_client_datagram(
        &mut self,
        proxy: &Proxy,
        services: &UdpRuntimeServices,
        dispatch: &mut UdpDispatch,
        relay: &TokioDatagramSocket,
        pending_control_traffic: &mut StreamTraffic,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<(), EngineError>;

    async fn write_direct_response(
        &mut self,
        services: &UdpRuntimeServices,
        dispatch: &UdpDispatch,
        relay: &TokioDatagramSocket,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<(), EngineError>;

    async fn write_upstream_response(
        &mut self,
        relay: &TokioDatagramSocket,
        response: &UdpUpstreamResponseParts,
    ) -> Result<usize, EngineError>;

    async fn write_chain_response(
        &mut self,
        relay: &TokioDatagramSocket,
        response: &UdpChainResponseParts,
    ) -> Result<usize, EngineError>;
}

impl<H> UdpAssociationHandler for H
where
    H: InboundUdpAssociation + InboundUdpAssociationResponder,
{
    async fn handle_client_datagram(
        &mut self,
        proxy: &Proxy,
        services: &UdpRuntimeServices,
        dispatch: &mut UdpDispatch,
        relay: &TokioDatagramSocket,
        pending_control_traffic: &mut StreamTraffic,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let inbound_tag = dispatch.inbound_tag().to_owned();
        let sender = zero_platform_tokio::socket_addr_to_socket_address(sender);
        let mut dispatch_bridge = UdpAssociationDispatchBridge {
            proxy,
            services: services.clone(),
            dispatch,
            pending_control_traffic,
            outcome: UdpAssociationDispatchOutcome::ClientHandled,
        };
        let result = match self
            .dispatch_datagram(sender, payload, &mut dispatch_bridge)
            .await
        {
            Err(error) => Err(error),
            Ok(()) => match dispatch_bridge.outcome {
                UdpAssociationDispatchOutcome::ClientHandled => Ok(()),
                UdpAssociationDispatchOutcome::PeerResponse { sender, payload } => {
                    let sender_socket_addr =
                        zero_platform_tokio::socket_address_to_socket_addr(sender);
                    let response = record_direct_udp_response_parts(
                        services,
                        dispatch_bridge.dispatch,
                        sender_socket_addr,
                        &payload,
                    );
                    write_direct_udp_response(&response, || async {
                        write_peer_response(self, relay, sender, &payload).await
                    })
                    .await?;
                    Ok(())
                }
                UdpAssociationDispatchOutcome::UnexpectedSender { sender } => {
                    tracing::debug!(?sender, "dropping udp packet from unexpected sender");
                    Ok(())
                }
            },
        };

        if let Err(error) = result {
            tracing::warn!(
                inbound_tag,
                protocol = "udp_association",
                error = %error,
                "failed to process UDP association packet"
            );
        }

        Ok(())
    }

    async fn write_direct_response(
        &mut self,
        services: &UdpRuntimeServices,
        dispatch: &UdpDispatch,
        relay: &TokioDatagramSocket,
        sender: SocketAddr,
        payload: &[u8],
    ) -> Result<(), EngineError> {
        let response = record_direct_udp_response_parts(services, dispatch, sender, payload);
        write_direct_udp_response(&response, || async {
            write_target_response(
                self,
                relay,
                &response.target,
                response.port,
                response.payload,
            )
            .await
        })
        .await?;

        Ok(())
    }

    async fn write_upstream_response(
        &mut self,
        relay: &TokioDatagramSocket,
        response: &UdpUpstreamResponseParts,
    ) -> Result<usize, EngineError> {
        write_target_response(
            self,
            relay,
            &response.target,
            response.port,
            &response.payload,
        )
        .await
    }

    async fn write_chain_response(
        &mut self,
        relay: &TokioDatagramSocket,
        response: &UdpChainResponseParts,
    ) -> Result<usize, EngineError> {
        write_target_response(
            self,
            relay,
            &response.target,
            response.port,
            &response.payload,
        )
        .await
    }
}

pub(crate) struct UdpAssociationLoopRequest<'a, S, H> {
    pub(crate) proxy: &'a Proxy,
    pub(crate) client: &'a mut MeteredStream<S>,
    pub(crate) inbound_tag: &'a str,
    pub(crate) relay: TokioDatagramSocket,
    pub(crate) pending_control_traffic: StreamTraffic,
    pub(crate) handler: H,
}

async fn write_target_response<H>(
    association: &H,
    relay: &TokioDatagramSocket,
    target: &zero_core::Address,
    port: u16,
    payload: &[u8],
) -> Result<usize, EngineError>
where
    H: InboundUdpAssociationResponder,
{
    send_association_response(
        relay,
        association.build_response_for_target(target, port, payload)?,
    )
    .await
}

async fn write_peer_response<H>(
    association: &H,
    relay: &TokioDatagramSocket,
    sender: zero_traits::SocketAddress,
    payload: &[u8],
) -> Result<usize, EngineError>
where
    H: InboundUdpAssociationResponder,
{
    send_association_response(relay, association.build_peer_response(sender, payload)?).await
}

async fn send_association_response(
    relay: &TokioDatagramSocket,
    response: Option<InboundUdpAssociationResponse>,
) -> Result<usize, EngineError> {
    let Some(response) = response else {
        return Ok(0);
    };
    let (recipient, payload) = response.into_parts();
    relay
        .send_to_addr(
            &payload,
            zero_platform_tokio::socket_address_to_socket_addr(recipient),
        )
        .await
        .map_err(EngineError::from)
}
