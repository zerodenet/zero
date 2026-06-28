use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::{AsyncSocket, DatagramSocket, IpAddress, UdpRelayProtocol};

use crate::outbound::{
    Socks5Outbound, Socks5OutboundAuth, Socks5OwnedOutboundAuth, Socks5UdpRelayTarget,
};
use crate::shared::{
    build_udp_packet, decode_udp_associate_request, decode_udp_associate_response,
    encode_udp_associate_response_to_client, Socks5InboundUdpRequest, Socks5InboundUdpResponse,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Socks5UdpRelayEndpoint {
    pub address: IpAddress,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpRelayTargetAddress {
    pub address: Address,
    pub port: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Socks5UdpAssociationConfig<'a> {
    auth: Option<Socks5OutboundAuth<'a>>,
}

impl<'a> Socks5UdpAssociationConfig<'a> {
    pub fn new(auth: Option<Socks5OutboundAuth<'a>>) -> Self {
        Self { auth }
    }

    pub fn auth(&self) -> Option<Socks5OutboundAuth<'a>> {
        self.auth
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5OwnedUdpAssociationConfig {
    auth: Option<Socks5OwnedOutboundAuth>,
}

impl Socks5OwnedUdpAssociationConfig {
    pub fn new(auth: Option<Socks5OwnedOutboundAuth>) -> Self {
        Self { auth }
    }

    pub fn as_ref(&self) -> Socks5UdpAssociationConfig<'_> {
        Socks5UdpAssociationConfig::new(self.auth.as_ref().map(|auth| auth.as_ref()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpAssociationTarget {
    outbound_tag: alloc::string::String,
    server: alloc::string::String,
    port: u16,
    config: Socks5OwnedUdpAssociationConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpAssociationIdentity {
    outbound_tag: alloc::string::String,
    server: alloc::string::String,
    port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpAssociationEndpoint {
    server: alloc::string::String,
    port: u16,
}

impl Socks5UdpAssociationTarget {
    pub fn new(
        outbound_tag: impl Into<alloc::string::String>,
        server: impl Into<alloc::string::String>,
        port: u16,
        config: Socks5OwnedUdpAssociationConfig,
    ) -> Self {
        Self {
            outbound_tag: outbound_tag.into(),
            server: server.into(),
            port,
            config,
        }
    }

    pub fn outbound_tag(&self) -> &str {
        &self.outbound_tag
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn association_config(&self) -> Socks5UdpAssociationConfig<'_> {
        self.config.as_ref()
    }

    pub fn identity(&self) -> Socks5UdpAssociationIdentity {
        Socks5UdpAssociationIdentity {
            outbound_tag: self.outbound_tag.clone(),
            server: self.server.clone(),
            port: self.port,
        }
    }

    pub fn connect_endpoint(&self) -> Socks5UdpAssociationEndpoint {
        Socks5UdpAssociationEndpoint {
            server: self.server.clone(),
            port: self.port,
        }
    }

    pub fn matches(&self, outbound_tag: &str, server: &str, port: u16) -> bool {
        self.outbound_tag == outbound_tag && self.server == server && self.port == port
    }
}

impl Socks5UdpAssociationIdentity {
    pub fn into_parts(self) -> (alloc::string::String, alloc::string::String, u16) {
        (self.outbound_tag, self.server, self.port)
    }
}

impl Socks5UdpAssociationEndpoint {
    pub fn into_parts(self) -> (alloc::string::String, u16) {
        (self.server, self.port)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Socks5UdpRelayError<E> {
    Socket(E),
    Protocol(Error),
}

#[derive(Debug)]
pub struct Socks5UdpRelay<S> {
    socket: S,
    endpoint: Socks5UdpRelayEndpoint,
}

#[derive(Debug)]
pub struct Socks5UdpAssociation<C, S> {
    _control: C,
    relay: Socks5UdpRelay<S>,
}

#[derive(Debug)]
pub struct Socks5EstablishedUdpAssociation<C, S> {
    target: Socks5UdpAssociationTarget,
    association: Socks5UdpAssociation<C, S>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Socks5InboundUdpCodec;

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
    ) -> Result<alloc::vec::Vec<u8>, Error> {
        encode_udp_associate_response_to_client(upstream_address, upstream_port, payload)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5InboundUdpResponseFrame {
    packet: Vec<u8>,
}

impl Socks5InboundUdpResponseFrame {
    pub fn len(&self) -> usize {
        self.packet.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packet.is_empty()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.packet
    }

    pub fn into_packet(self) -> Vec<u8> {
        self.packet
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5InboundUdpResponseKey {
    target: Address,
    port: u16,
}

impl Socks5InboundUdpResponseKey {
    pub fn target(&self) -> &Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Socks5InboundUdpSession {
    codec: Socks5InboundUdpCodec,
}

impl Socks5InboundUdpSession {
    pub fn new() -> Self {
        Self {
            codec: Socks5InboundUdpCodec,
        }
    }

    pub fn decode_request(&self, packet: &[u8]) -> Result<Socks5InboundUdpRequest, Error> {
        self.codec.decode_request(packet)
    }

    pub fn decode_response(&self, packet: &[u8]) -> Result<Socks5InboundUdpResponse, Error> {
        self.codec.decode_response(packet)
    }

    pub fn encode_response_to_client(
        &self,
        upstream_address: &Address,
        upstream_port: u16,
        payload: &[u8],
    ) -> Result<alloc::vec::Vec<u8>, Error> {
        self.codec
            .encode_response_to_client(upstream_address, upstream_port, payload)
    }

    pub fn response_frame(
        &self,
        upstream_address: &Address,
        upstream_port: u16,
        payload: &[u8],
    ) -> Result<Socks5InboundUdpResponseFrame, Error> {
        Ok(Socks5InboundUdpResponseFrame {
            packet: self.encode_response_to_client(upstream_address, upstream_port, payload)?,
        })
    }

    pub fn response_key(&self, packet: &[u8]) -> Result<Socks5InboundUdpResponseKey, Error> {
        let response = self.decode_response(packet)?;
        Ok(Socks5InboundUdpResponseKey {
            target: response.target().clone(),
            port: response.port(),
        })
    }
}

impl<C, S> Socks5UdpAssociation<C, S> {
    pub fn new(control: C, relay: Socks5UdpRelay<S>) -> Self {
        Self {
            _control: control,
            relay,
        }
    }

    pub fn from_relay_socket(control: C, socket: S, endpoint: Socks5UdpRelayEndpoint) -> Self {
        Self::new(control, Socks5UdpRelay::new(socket, endpoint))
    }

    pub fn from_relay_endpoint(control: C, socket: S, address: IpAddress, port: u16) -> Self {
        Self::from_relay_socket(control, socket, Socks5UdpRelayEndpoint { address, port })
    }

    pub fn relay(&self) -> &Socks5UdpRelay<S> {
        &self.relay
    }

    pub fn into_parts(self) -> (C, Socks5UdpRelay<S>) {
        (self._control, self.relay)
    }
}

impl<C, S> Socks5EstablishedUdpAssociation<C, S> {
    pub fn new(
        target: Socks5UdpAssociationTarget,
        association: Socks5UdpAssociation<C, S>,
    ) -> Self {
        Self {
            target,
            association,
        }
    }

    pub fn from_relay_endpoint(
        target: Socks5UdpAssociationTarget,
        control: C,
        socket: S,
        address: IpAddress,
        port: u16,
    ) -> Self {
        Self::new(
            target,
            Socks5UdpAssociation::from_relay_endpoint(control, socket, address, port),
        )
    }

    pub fn target(&self) -> &Socks5UdpAssociationTarget {
        &self.target
    }

    pub fn outbound_tag(&self) -> &str {
        self.target.outbound_tag()
    }

    pub fn upstream_endpoint(&self) -> (&str, u16) {
        (self.target.server(), self.target.port())
    }

    pub fn into_parts(self) -> (Socks5UdpAssociationTarget, Socks5UdpAssociation<C, S>) {
        (self.target, self.association)
    }
}

impl<C, S> Socks5EstablishedUdpAssociation<C, S>
where
    S: DatagramSocket,
{
    pub async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        self.association.send_packet(target, port, payload).await
    }

    pub async fn recv_packet(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        self.association.recv_packet(buf).await
    }

    pub async fn recv_payload(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        self.association.recv_payload(buf).await
    }
}

impl<S> Socks5UdpRelay<S> {
    pub fn new(socket: S, endpoint: Socks5UdpRelayEndpoint) -> Self {
        Self { socket, endpoint }
    }

    pub fn endpoint(&self) -> Socks5UdpRelayEndpoint {
        self.endpoint
    }

    pub fn socket(&self) -> &S {
        &self.socket
    }

    pub fn into_socket(self) -> S {
        self.socket
    }
}

impl<C, S> Socks5UdpAssociation<C, S>
where
    S: DatagramSocket,
{
    pub async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        self.relay.send_packet(target, port, payload).await
    }

    pub async fn recv_packet(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        self.relay.recv_packet(buf).await
    }

    pub async fn recv_payload(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        self.relay.recv_payload(buf).await
    }
}

pub async fn establish_udp_relay_with_control<S>(
    control_stream: &mut S,
    config: Socks5UdpAssociationConfig<'_>,
) -> Result<Socks5UdpRelayTargetAddress, Error>
where
    S: AsyncSocket,
{
    let (address, port) = Socks5Outbound
        .establish_udp_relay(
            control_stream,
            &Socks5UdpRelayTarget {
                auth: config.auth(),
            },
        )
        .await?;
    Ok(Socks5UdpRelayTargetAddress { address, port })
}

impl<S> Socks5UdpRelay<S>
where
    S: DatagramSocket,
{
    pub async fn send_packet(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        let packet =
            build_udp_packet(target, port, payload).map_err(Socks5UdpRelayError::Protocol)?;
        self.socket
            .send_to(&packet, self.endpoint.address, self.endpoint.port)
            .await
            .map_err(Socks5UdpRelayError::Socket)?;

        Ok(packet.len())
    }

    pub async fn recv_packet(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        let (read, address, port) = self
            .socket
            .recv_from(buf)
            .await
            .map_err(Socks5UdpRelayError::Socket)?;

        if address != self.endpoint.address || port != self.endpoint.port {
            return Err(Socks5UdpRelayError::Protocol(Error::Protocol(
                "unexpected UDP sender from SOCKS5 upstream",
            )));
        }

        Ok(read)
    }

    pub async fn recv_payload(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        let read = self.recv_packet(buf).await?;
        let packet =
            decode_udp_associate_response(&buf[..read]).map_err(Socks5UdpRelayError::Protocol)?;
        let payload_len = packet.payload().len();
        buf[..payload_len].copy_from_slice(packet.payload());
        Ok(payload_len)
    }
}

impl<E> From<Error> for Socks5UdpRelayError<E> {
    fn from(error: Error) -> Self {
        Self::Protocol(error)
    }
}
