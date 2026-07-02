use alloc::vec::Vec;

use zero_core::{Address, Error};

use super::packet::{
    decode_udp_associate_request, decode_udp_associate_response,
    encode_udp_associate_response_to_client, Socks5InboundUdpDispatchAction,
    Socks5InboundUdpDispatchView, Socks5InboundUdpRequest, Socks5InboundUdpResponse,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Socks5InboundUdpCodec;

pub trait Socks5InboundUdpDispatchActionDispatcher {
    type Error;

    async fn dispatch_local_dns(&mut self, domain: &str) -> Result<(), Self::Error>;

    async fn dispatch_inbound_packet(
        &mut self,
        view: Socks5InboundUdpDispatchView,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Socks5InboundUdpSession {
    codec: Socks5InboundUdpCodec,
}

impl Socks5InboundUdpCodec {
    pub fn decode_request(&self, packet: &[u8]) -> Result<Socks5InboundUdpRequest, Error> {
        decode_udp_associate_request(packet)
            .map(|decoded| Socks5InboundUdpRequest::from_packet(decoded, packet.len()))
    }

    pub fn decode_response(&self, packet: &[u8]) -> Result<Socks5InboundUdpResponse, Error> {
        decode_udp_associate_response(packet).map(Socks5InboundUdpResponse::from_packet)
    }

    pub fn encode_response_to_client(
        &self,
        upstream_address: &Address,
        upstream_port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        encode_udp_associate_response_to_client(upstream_address, upstream_port, payload)
    }
}

impl Socks5InboundUdpSession {
    pub(crate) fn new() -> Self {
        Self {
            codec: Socks5InboundUdpCodec,
        }
    }

    pub(crate) fn decode_request(&self, packet: &[u8]) -> Result<Socks5InboundUdpRequest, Error> {
        self.codec.decode_request(packet)
    }

    fn decode_dispatch_action(
        &self,
        packet: &[u8],
    ) -> Result<Socks5InboundUdpDispatchAction, Error> {
        self.decode_request(packet)
            .map(Socks5InboundUdpRequest::into_dispatch_action)
    }

    pub(crate) async fn dispatch_client_packet<D>(
        &self,
        packet: &[u8],
        dispatcher: &mut D,
    ) -> Result<(), D::Error>
    where
        D: Socks5InboundUdpDispatchActionDispatcher,
        D::Error: From<Error>,
    {
        self.decode_dispatch_action(packet)
            .map_err(D::Error::from)?
            .dispatch_with(dispatcher)
            .await
    }

    pub(crate) fn encode_response_to_client(
        &self,
        upstream_address: &Address,
        upstream_port: u16,
        payload: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.codec
            .encode_response_to_client(upstream_address, upstream_port, payload)
    }
}

impl Socks5InboundUdpDispatchAction {
    pub(crate) async fn dispatch_with<D>(self, dispatcher: &mut D) -> Result<(), D::Error>
    where
        D: Socks5InboundUdpDispatchActionDispatcher,
    {
        match self {
            Self::LocalDns { domain } => dispatcher.dispatch_local_dns(&domain).await,
            Self::Dispatch(view) => dispatcher.dispatch_inbound_packet(view).await,
        }
    }
}
