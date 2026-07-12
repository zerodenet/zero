use alloc::vec::Vec;
use zero_core::{Address, Error};

use super::packet::{
    decode_udp_associate_request, decode_udp_associate_response,
    encode_udp_associate_response_to_client, Socks5InboundUdpRequest, Socks5InboundUdpResponse,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Socks5InboundUdpCodec;

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
