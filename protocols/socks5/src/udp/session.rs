use zero_core::{Address, Error};
use zero_traits::{DatagramSocket, IpAddress, SocketAddress};

use super::association::Socks5UdpRelayError;
use super::dispatch::{Socks5InboundUdpDispatchActionDispatcher, Socks5InboundUdpSession};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Socks5InboundUdpResponder {
    session: Socks5InboundUdpSession,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Socks5InboundUdpAssociationSession {
    relay_session: Socks5InboundUdpRelaySession,
    responder: Socks5InboundUdpResponder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Socks5InboundUdpRelayPacketAction<'a> {
    ClientPacket {
        payload: &'a [u8],
    },
    PeerResponse {
        sender: SocketAddress,
        payload: &'a [u8],
    },
    UnexpectedSender {
        sender: SocketAddress,
    },
}

pub trait Socks5InboundUdpRelayPacketDispatcher {
    type Error;

    async fn dispatch_client_packet(&mut self, payload: &[u8]) -> Result<(), Self::Error>;

    async fn dispatch_peer_response(
        &mut self,
        sender: SocketAddress,
        payload: &[u8],
    ) -> Result<(), Self::Error>;

    async fn dispatch_unexpected_sender(
        &mut self,
        sender: SocketAddress,
    ) -> Result<(), Self::Error>;
}

impl crate::inbound::Socks5UdpAssociateRequest {
    pub fn client_endpoint_hint(&self) -> Option<SocketAddress> {
        if self.client_port == 0 {
            return None;
        }

        match &self.client_hint {
            Address::Ipv4(ip) if *ip != [0, 0, 0, 0] => Some(SocketAddress {
                ip: IpAddress::V4(*ip),
                port: self.client_port,
            }),
            Address::Ipv6(ip) if *ip != [0; 16] => Some(SocketAddress {
                ip: IpAddress::V6(*ip),
                port: self.client_port,
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Socks5InboundUdpRelaySession {
    client: Option<SocketAddress>,
}

impl Socks5InboundUdpRelaySession {
    fn new() -> Self {
        Self::default()
    }

    fn from_associate_request(request: &crate::inbound::Socks5UdpAssociateRequest) -> Self {
        Self {
            client: request.client_endpoint_hint(),
        }
    }

    fn client(&self) -> Option<SocketAddress> {
        self.client
    }

    fn classify_packet<'a>(
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
            Some(_client) => Socks5InboundUdpRelayPacketAction::PeerResponse { sender, payload },
            None => Socks5InboundUdpRelayPacketAction::UnexpectedSender { sender },
        }
    }

    async fn dispatch_packet<D>(
        &mut self,
        sender: SocketAddress,
        payload: &[u8],
        dispatcher: &mut D,
    ) -> Result<(), D::Error>
    where
        D: Socks5InboundUdpRelayPacketDispatcher,
    {
        self.classify_packet(sender, payload)
            .dispatch_with(dispatcher)
            .await
    }
}

impl Socks5InboundUdpResponder {
    fn new() -> Self {
        Self {
            session: Socks5InboundUdpSession::new(),
        }
    }

    async fn send_client_response_for_target<S>(
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
        self.session
            .send_client_response_for_target(
                socket,
                client,
                upstream_address,
                upstream_port,
                payload,
            )
            .await
    }
}

impl Socks5InboundUdpAssociationSession {
    pub fn new() -> Self {
        Self {
            relay_session: Socks5InboundUdpRelaySession::new(),
            responder: Socks5InboundUdpResponder::new(),
        }
    }

    pub fn from_associate_request(request: crate::inbound::Socks5UdpAssociateRequest) -> Self {
        Self {
            relay_session: Socks5InboundUdpRelaySession::from_associate_request(&request),
            responder: Socks5InboundUdpResponder::new(),
        }
    }

    fn client(&self) -> Option<SocketAddress> {
        self.relay_session.client()
    }

    pub async fn dispatch_relay_packet<D>(
        &mut self,
        sender: SocketAddress,
        payload: &[u8],
        dispatcher: &mut D,
    ) -> Result<(), D::Error>
    where
        D: Socks5InboundUdpRelayPacketDispatcher,
    {
        self.relay_session
            .dispatch_packet(sender, payload, dispatcher)
            .await
    }

    pub async fn dispatch_client_packet<D>(
        &self,
        packet: &[u8],
        dispatcher: &mut D,
    ) -> Result<(), D::Error>
    where
        D: Socks5InboundUdpDispatchActionDispatcher,
        D::Error: From<Error>,
    {
        self.responder
            .session
            .dispatch_client_packet(packet, dispatcher)
            .await
    }

    async fn send_client_response_for_target<S>(
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
        self.responder
            .send_client_response_for_target(
                socket,
                client,
                upstream_address,
                upstream_port,
                payload,
            )
            .await
    }

    pub async fn send_current_client_response_for_target<S>(
        &self,
        socket: &S,
        upstream_address: &Address,
        upstream_port: u16,
        payload: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>>
    where
        S: DatagramSocket,
    {
        let Some(client) = self.client() else {
            return Ok(0);
        };
        self.send_client_response_for_target(
            socket,
            client,
            upstream_address,
            upstream_port,
            payload,
        )
        .await
    }

    pub async fn send_current_client_peer_response_parts<S>(
        &self,
        socket: &S,
        sender: SocketAddress,
        payload: &[u8],
    ) -> Result<usize, Socks5UdpRelayError<S::Error>>
    where
        S: DatagramSocket,
    {
        let Some(client) = self.client() else {
            return Ok(0);
        };
        let (target, port) = socket_address_response_target(sender);
        self.send_client_response_for_target(socket, client, &target, port, payload)
            .await
    }
}

impl crate::inbound::Socks5Inbound {
    pub fn accept_udp_association(
        &self,
        request: crate::inbound::Socks5UdpAssociateRequest,
    ) -> Socks5InboundUdpAssociationSession {
        Socks5InboundUdpAssociationSession::from_associate_request(request)
    }
}

impl Socks5InboundUdpSession {
    async fn send_response_to_client<S>(
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
        let packet = self
            .encode_response_to_client(upstream_address, upstream_port, payload)
            .map_err(Socks5UdpRelayError::Protocol)?;
        socket
            .send_to(&packet, client, client_port)
            .await
            .map_err(Socks5UdpRelayError::Socket)?;
        Ok(packet.len())
    }

    async fn send_client_response_for_target<S>(
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
}

impl<'a> Socks5InboundUdpRelayPacketAction<'a> {
    async fn dispatch_with<D>(self, dispatcher: &mut D) -> Result<(), D::Error>
    where
        D: Socks5InboundUdpRelayPacketDispatcher,
    {
        match self {
            Self::ClientPacket { payload } => dispatcher.dispatch_client_packet(payload).await,
            Self::PeerResponse { sender, payload } => {
                dispatcher.dispatch_peer_response(sender, payload).await
            }
            Self::UnexpectedSender { sender } => {
                dispatcher.dispatch_unexpected_sender(sender).await
            }
        }
    }
}

fn socket_address_response_target(sender: SocketAddress) -> (Address, u16) {
    let target = match sender.ip {
        IpAddress::V4(ip) => Address::Ipv4(ip),
        IpAddress::V6(ip) => Address::Ipv6(ip),
    };
    (target, sender.port)
}
