use zero_core::{Address, Error};
use zero_traits::{AsyncSocket, DatagramSocket, IpAddress, UdpRelayProtocol};

use crate::outbound::{Socks5Outbound, Socks5OutboundAuth, Socks5UdpRelayTarget};
use crate::shared::{build_udp_packet, decode_udp_associate_response};

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

impl<C, S> Socks5UdpAssociation<C, S> {
    pub fn new(control: C, relay: Socks5UdpRelay<S>) -> Self {
        Self {
            _control: control,
            relay,
        }
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
    auth: Option<(&str, &str)>,
) -> Result<Socks5UdpRelayTargetAddress, Error>
where
    S: AsyncSocket,
{
    let (address, port) = Socks5Outbound
        .establish_udp_relay(
            control_stream,
            &Socks5UdpRelayTarget {
                auth: auth.map(|(username, password)| Socks5OutboundAuth { username, password }),
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
