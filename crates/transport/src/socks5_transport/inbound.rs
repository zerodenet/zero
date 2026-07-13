use std::future::Future;
use std::net::SocketAddr;

use zero_config::{InboundProtocolConfig, Socks5UserConfig};
use zero_core::{
    Address, InboundClientResponse, InboundUdpAssociation, InboundUdpAssociationDispatcher,
    InboundUdpAssociationResponder, InboundUdpAssociationResponse, Session,
};
use zero_engine::EngineError;
use zero_platform_tokio::TokioDatagramSocket;
use zero_traits::{AsyncSocket, SocketAddress};

use super::{
    OwnedSocks5InboundAcceptor, Socks5InboundUdpAssociationHandler,
    Socks5InboundUdpAssociationSetup,
};
use crate::{ClientStream, MeteredStream};

pub fn inbound_acceptor_from_users(users: &[Socks5UserConfig]) -> OwnedSocks5InboundAcceptor {
    OwnedSocks5InboundAcceptor::new(socks5::Socks5InboundTcpAcceptor::from_config_users(
        users.iter().map(|user| {
            (
                user.username.as_str(),
                user.password.as_str(),
                user.principal_key.as_deref(),
                user.up_bps,
                user.down_bps,
            )
        }),
    ))
}

pub fn inbound_acceptor_from_protocol(
    protocol: &InboundProtocolConfig,
) -> Result<OwnedSocks5InboundAcceptor, EngineError> {
    match protocol {
        InboundProtocolConfig::Socks5 { users } => Ok(inbound_acceptor_from_users(users)),
        _ => Err(EngineError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "socks5 inbound acceptor received non-socks5 inbound config",
        ))),
    }
}

pub async fn setup_inbound_udp_association<S>(
    client: &mut MeteredStream<S>,
    request: socks5::udp::Socks5UdpAssociateRequest,
) -> Result<Socks5InboundUdpAssociationSetup, EngineError>
where
    S: ClientStream,
{
    let control_local_addr = client.local_addr()?;
    let relay = TokioDatagramSocket::bind_addr(SocketAddr::new(control_local_addr.ip(), 0)).await?;
    let relay_addr = relay.local_addr()?;
    let relay_bind = match zero_platform_tokio::socket_addr_to_ip(relay_addr) {
        zero_traits::IpAddress::V4(ip) => Address::Ipv4(ip),
        zero_traits::IpAddress::V6(ip) => Address::Ipv6(ip),
    };
    socks5::Socks5Inbound
        .send_success_response_with_bound(client, &relay_bind, relay_addr.port())
        .await?;
    Ok(Socks5InboundUdpAssociationSetup {
        relay,
        pending_control_traffic: client.drain_traffic(),
        handler: Socks5InboundUdpAssociationHandler::new(
            socks5::Socks5Inbound.accept_udp_association(request),
        ),
    })
}

impl OwnedSocks5InboundAcceptor {
    fn new(protocol: socks5::Socks5InboundTcpAcceptor) -> Self {
        Self { protocol }
    }

    pub async fn accept_and_dispatch_command<S, Connect, ConnectFut, Udp, UdpFut, E>(
        &self,
        stream: MeteredStream<S>,
        on_connect: Connect,
        on_udp_associate: Udp,
    ) -> Result<(), E>
    where
        S: ClientStream,
        Connect: FnOnce(Session, S) -> ConnectFut,
        ConnectFut: Future<Output = Result<(), E>>,
        Udp: FnOnce(Socks5InboundUdpAssociationSetup, MeteredStream<S>) -> UdpFut,
        UdpFut: Future<Output = Result<(), E>>,
        E: From<zero_core::Error> + From<EngineError>,
    {
        self.protocol
            .accept_and_dispatch_command_with(
                stream,
                |session, stream| async move { on_connect(session, stream.into_inner()).await },
                |request, mut stream| async move {
                    let setup = setup_inbound_udp_association(&mut stream, request)
                        .await
                        .map_err(E::from)?;
                    on_udp_associate(setup, stream).await
                },
            )
            .await
    }
}

impl<S> InboundClientResponse<S> for OwnedSocks5InboundAcceptor
where
    S: AsyncSocket,
{
    async fn send_ok(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_success(client).await
    }

    async fn send_blocked(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_blocked(client).await
    }

    async fn send_upstream_failure(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_upstream_failure(client).await
    }
}

impl Socks5InboundUdpAssociationHandler {
    fn new(protocol: socks5::udp::Socks5InboundUdpAssociationSession) -> Self {
        Self { protocol }
    }
}

impl InboundUdpAssociation for Socks5InboundUdpAssociationHandler {
    async fn dispatch_datagram<D>(
        &mut self,
        sender: SocketAddress,
        packet: &[u8],
        dispatcher: &mut D,
    ) -> Result<(), D::Error>
    where
        D: InboundUdpAssociationDispatcher,
        D::Error: From<zero_core::Error>,
    {
        self.protocol
            .dispatch_datagram(sender, packet, dispatcher)
            .await
    }
}

impl InboundUdpAssociationResponder for Socks5InboundUdpAssociationHandler {
    fn build_response_for_target(
        &self,
        target: &Address,
        port: u16,
        payload: &[u8],
    ) -> Result<Option<InboundUdpAssociationResponse>, zero_core::Error> {
        self.protocol
            .build_response_for_target(target, port, payload)
    }

    fn build_peer_response(
        &self,
        sender: SocketAddress,
        payload: &[u8],
    ) -> Result<Option<InboundUdpAssociationResponse>, zero_core::Error> {
        self.protocol.build_peer_response(sender, payload)
    }
}
