use alloc::vec::Vec;

use zero_core::{Address, Error};
use zero_traits::{
    AsyncSocket, DatagramSocket, DnsResolver, IpAddress, SocketAddress, UdpRelayProtocol,
};

use crate::outbound::{Socks5Outbound, Socks5OutboundAuth, Socks5OwnedOutboundAuth};
use crate::shared::{
    build_udp_packet, decode_udp_associate_request, decode_udp_associate_response,
    encode_udp_associate_response_to_client, Socks5UdpPacket,
};

pub use crate::inbound::Socks5UdpAssociateRequest;
pub use crate::outbound::{
    Socks5UdpAssociationSend, Socks5UdpFlowResume, Socks5UdpFlowSpec, Socks5UdpRelayTarget,
};
pub use crate::shared::{
    packet_path_carrier_association_target, udp_flow_resume_from_config,
    udp_packet_path_carrier_build_from_config, udp_packet_path_carrier_descriptor_from_config,
    udp_packet_path_spec_from_config, Socks5InboundUdpDispatchAction,
    Socks5InboundUdpDispatchParts, Socks5InboundUdpDispatchView, Socks5InboundUdpRequest,
    Socks5InboundUdpResponse, Socks5UdpFlowConfig, Socks5UdpPacketPathCarrierBuild,
    Socks5UdpPacketPathCarrierDescriptor, Socks5UdpPacketPathSpec,
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
pub struct Socks5UdpAssociationLifecycleRecord {
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

    pub fn outbound_tag(&self) -> &str {
        &self.outbound_tag
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn matches(&self, other: &Self) -> bool {
        self.outbound_tag == other.outbound_tag
            && self.server == other.server
            && self.port == other.port
    }

    pub fn lifecycle_record(&self) -> Socks5UdpAssociationLifecycleRecord {
        Socks5UdpAssociationLifecycleRecord {
            outbound_tag: self.outbound_tag.clone(),
            server: self.server.clone(),
            port: self.port,
        }
    }
}

impl Socks5UdpAssociationLifecycleRecord {
    pub fn outbound_tag(&self) -> &str {
        &self.outbound_tag
    }

    pub fn server(&self) -> &str {
        &self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

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

#[derive(Debug, Clone, Copy)]
pub struct Socks5UdpClientResponse<'a> {
    upstream_address: &'a Address,
    upstream_port: u16,
    payload: &'a [u8],
}

impl Socks5InboundUdpResponseKey {
    pub fn target(&self) -> &Address {
        &self.target
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn into_parts(self) -> (Address, u16) {
        (self.target, self.port)
    }
}

impl<'a> Socks5UdpClientResponse<'a> {
    pub fn new(upstream_address: &'a Address, upstream_port: u16, payload: &'a [u8]) -> Self {
        Self {
            upstream_address,
            upstream_port,
            payload,
        }
    }

    pub fn payload_len(&self) -> usize {
        self.payload.len()
    }

    pub fn upstream_address(&self) -> &'a Address {
        self.upstream_address
    }

    pub fn upstream_port(&self) -> u16 {
        self.upstream_port
    }

    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Socks5InboundUdpSession {
    codec: Socks5InboundUdpCodec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Socks5InboundUdpRelayPacketAction<'a> {
    ClientPacket {
        payload: &'a [u8],
    },
    PeerResponse {
        client: SocketAddress,
        sender: SocketAddress,
        payload: &'a [u8],
    },
    UnexpectedSender {
        sender: SocketAddress,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Socks5InboundUdpRelaySession {
    client: Option<SocketAddress>,
}

impl Socks5InboundUdpRelaySession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn client(&self) -> Option<SocketAddress> {
        self.client
    }

    pub fn classify_packet<'a>(
        &mut self,
        sender: SocketAddress,
        payload: &'a [u8],
    ) -> Socks5InboundUdpRelayPacketAction<'a> {
        if self.client.is_none() {
            self.client = Some(sender);
        }

        match self.client {
            Some(client) if client == sender => {
                Socks5InboundUdpRelayPacketAction::ClientPacket { payload }
            }
            Some(client) => Socks5InboundUdpRelayPacketAction::PeerResponse {
                client,
                sender,
                payload,
            },
            None => Socks5InboundUdpRelayPacketAction::UnexpectedSender { sender },
        }
    }
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

    pub fn decode_dispatch_parts(
        &self,
        packet: &[u8],
    ) -> Result<Socks5InboundUdpDispatchParts, Error> {
        self.decode_request(packet)
            .map(Socks5InboundUdpRequest::into_dispatch_parts)
    }

    pub fn decode_dispatch_action(
        &self,
        packet: &[u8],
    ) -> Result<Socks5InboundUdpDispatchAction, Error> {
        self.decode_request(packet)
            .map(Socks5InboundUdpRequest::into_dispatch_action)
    }

    pub async fn decode_dispatch_parts_or_resolve_local_dns<R>(
        &self,
        packet: &[u8],
        resolver: &R,
    ) -> Result<Option<Socks5InboundUdpDispatchView>, Error>
    where
        R: DnsResolver + ?Sized,
    {
        match self.decode_dispatch_action(packet)? {
            Socks5InboundUdpDispatchAction::LocalDns { domain } => {
                let _ = resolver.resolve(&domain).await;
                Ok(None)
            }
            Socks5InboundUdpDispatchAction::Dispatch(view) => Ok(Some(view)),
        }
    }

    pub fn request_dispatch_parts(
        &self,
        request: Socks5InboundUdpRequest,
    ) -> Socks5InboundUdpDispatchParts {
        request.into_dispatch_parts()
    }

    pub fn local_dns_domain_request<'a>(
        &self,
        request: &'a Socks5InboundUdpRequest,
    ) -> Option<&'a str> {
        request.dns_domain_request()
    }

    pub fn decode_response(&self, packet: &[u8]) -> Result<Socks5InboundUdpResponse, Error> {
        self.codec.decode_response(packet)
    }

    pub fn decode_response_parts(&self, packet: &[u8]) -> Result<(Address, u16, Vec<u8>), Error> {
        self.decode_response(packet)
            .map(Socks5InboundUdpResponse::into_parts)
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
        let (target, port, _) = response.into_parts();
        Ok(Socks5InboundUdpResponseKey { target, port })
    }

    pub fn response_session_key_parts(&self, packet: &[u8]) -> Result<(Address, u16), Error> {
        self.response_key(packet).map(|key| key.into_parts())
    }

    pub async fn send_encoded_response_to_client<S>(
        &self,
        socket: &S,
        client: SocketAddress,
        packet: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>>
    where
        S: DatagramSocket,
    {
        socket
            .send_to(packet, client.ip, client.port)
            .await
            .map_err(Socks5UdpRelayError::Socket)?;
        Ok(packet.len())
    }

    pub async fn send_response_to_client<S>(
        &self,
        socket: &S,
        client: IpAddress,
        client_port: u16,
        upstream_address: &Address,
        upstream_port: u16,
        payload: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>>
    where
        S: DatagramSocket,
    {
        let frame = self
            .response_frame(upstream_address, upstream_port, payload)
            .map_err(Socks5UdpRelayError::Protocol)?;
        let frame_len = frame.len();
        socket
            .send_to(frame.as_slice(), client, client_port)
            .await
            .map_err(Socks5UdpRelayError::Socket)?;
        Ok(frame_len)
    }

    pub async fn send_response_to_client_endpoint<S>(
        &self,
        socket: &S,
        client: IpAddress,
        client_port: u16,
        upstream: Socks5UdpRelayEndpoint,
        payload: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>>
    where
        S: DatagramSocket,
    {
        let upstream_address = address_from_ip(upstream.address);
        self.send_response_to_client(
            socket,
            client,
            client_port,
            &upstream_address,
            upstream.port,
            payload,
        )
        .await
    }

    pub async fn send_response_to_client_target<S>(
        &self,
        socket: &S,
        client: SocketAddress,
        upstream_address: &Address,
        upstream_port: u16,
        payload: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>>
    where
        S: DatagramSocket,
    {
        self.send_response_to_client(
            socket,
            client.ip,
            client.port,
            upstream_address,
            upstream_port,
            payload,
        )
        .await
    }

    pub async fn send_client_response<S>(
        &self,
        socket: &S,
        client: SocketAddress,
        response: Socks5UdpClientResponse<'_>,
    ) -> Result<usize, Socks5UdpRelayError<S::Error>>
    where
        S: DatagramSocket,
    {
        self.send_response_to_client_target(
            socket,
            client,
            response.upstream_address(),
            response.upstream_port(),
            response.payload(),
        )
        .await
    }

    pub async fn send_response_to_client_socket_addr<S>(
        &self,
        socket: &S,
        client: SocketAddress,
        upstream: SocketAddress,
        payload: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>>
    where
        S: DatagramSocket,
    {
        self.send_response_to_client_endpoint(
            socket,
            client.ip,
            client.port,
            Socks5UdpRelayEndpoint {
                address: upstream.ip,
                port: upstream.port,
            },
            payload,
        )
        .await
    }
}

fn address_from_ip(ip: IpAddress) -> Address {
    match ip {
        IpAddress::V4(bytes) => Address::Ipv4(bytes),
        IpAddress::V6(bytes) => Address::Ipv6(bytes),
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

    pub fn from_relay_socket_address(control: C, socket: S, endpoint: SocketAddress) -> Self {
        Self::from_relay_endpoint(control, socket, endpoint.ip, endpoint.port)
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

    pub fn from_relay_socket_address(
        target: Socks5UdpAssociationTarget,
        control: C,
        socket: S,
        endpoint: SocketAddress,
    ) -> Self {
        Self::from_relay_endpoint(target, control, socket, endpoint.ip, endpoint.port)
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

    pub fn identity(&self) -> Socks5UdpAssociationIdentity {
        self.target.identity()
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

    pub async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), Socks5UdpRelayError<S::Error>> {
        self.association.recv_response_parts(buf).await
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

    pub async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), Socks5UdpRelayError<S::Error>> {
        self.relay.recv_response_parts(buf).await
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

    pub async fn recv_response_parts(
        &self,
        buf: &mut [u8],
    ) -> Result<(Address, u16, Vec<u8>), Socks5UdpRelayError<S::Error>> {
        let read = self.recv_packet(buf).await?;
        decode_udp_associate_response(&buf[..read])
            .map(Socks5UdpPacket::into_parts)
            .map_err(Socks5UdpRelayError::Protocol)
    }

    pub async fn recv_payload(
        &self,
        buf: &mut [u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>> {
        let (_, _, payload) = self.recv_response_parts(buf).await?;
        let payload_len = payload.len();
        buf[..payload_len].copy_from_slice(&payload);
        Ok(payload_len)
    }
}

impl<E> From<Error> for Socks5UdpRelayError<E> {
    fn from(error: Error) -> Self {
        Self::Protocol(error)
    }
}

impl<E> Socks5UdpRelayError<E> {
    pub fn into_mapped<M, F>(self, map_socket: F) -> M
    where
        M: From<Error>,
        F: FnOnce(E) -> M,
    {
        match self {
            Self::Socket(error) => map_socket(error),
            Self::Protocol(error) => M::from(error),
        }
    }
}
