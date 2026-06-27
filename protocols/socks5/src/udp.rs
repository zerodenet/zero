use zero_core::{Address, Error};
use zero_traits::{AsyncSocket, DatagramSocket, IpAddress, UdpRelayProtocol};

use crate::outbound::{
    Socks5Outbound, Socks5OutboundAuth, Socks5OwnedOutboundAuth, Socks5UdpRelayTarget,
};
use crate::shared::{
    build_udp_packet, decode_udp_associate_request, decode_udp_associate_response,
    encode_udp_associate_response_to_client, Socks5UdpPacket,
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

    pub fn matches(&self, outbound_tag: &str, server: &str, port: u16) -> bool {
        self.outbound_tag == outbound_tag && self.server == server && self.port == port
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Socks5InboundUdpCodec;

impl Socks5InboundUdpCodec {
    pub fn decode_request(&self, packet: &[u8]) -> Result<Socks5UdpPacket, Error> {
        decode_udp_associate_request(packet)
    }

    pub fn decode_response(&self, packet: &[u8]) -> Result<Socks5UdpPacket, Error> {
        decode_udp_associate_response(packet)
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
        let payload_len = packet.payload.len();
        buf[..payload_len].copy_from_slice(&packet.payload);
        Ok(payload_len)
    }
}

impl<E> From<Error> for Socks5UdpRelayError<E> {
    fn from(error: Error) -> Self {
        Self::Protocol(error)
    }
}
