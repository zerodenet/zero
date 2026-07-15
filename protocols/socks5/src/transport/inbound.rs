use std::future::Future;
use std::net::SocketAddr;

use zero_core::{
    Address, InboundClientResponse, InboundUdpAssociation, InboundUdpAssociationDispatcher,
    InboundUdpAssociationResponder, InboundUdpAssociationResponse, Session,
};
use zero_platform_tokio::TokioDatagramSocket;
use zero_traits::{AsyncSocket, SocketAddress};
use zero_transport::RuntimeError;
use zero_transport::{ClientStream, MeteredStream};

use super::{
    Socks5InboundAcceptor, Socks5InboundUdpAssociationHandler, Socks5InboundUdpAssociationSetup,
    Socks5InboundUserRef,
};

pub async fn setup_inbound_udp_association<S>(
    client: &mut MeteredStream<S>,
    request: crate::udp::Socks5UdpAssociateRequest,
) -> Result<Socks5InboundUdpAssociationSetup, RuntimeError>
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
    crate::Socks5Inbound
        .send_success_response_with_bound(client, &relay_bind, relay_addr.port())
        .await?;
    Ok(Socks5InboundUdpAssociationSetup {
        relay,
        pending_control_traffic: client.drain_traffic(),
        handler: Socks5InboundUdpAssociationHandler::new(
            crate::Socks5Inbound.accept_udp_association(request),
        ),
    })
}

impl Socks5InboundAcceptor {
    pub fn from_options_refs<'a, I>(users: I) -> Self
    where
        I: IntoIterator<Item = Socks5InboundUserRef<'a>>,
    {
        Self::new(crate::Socks5InboundTcpAcceptor::from_config_users(
            users.into_iter().map(|user| {
                (
                    user.username,
                    user.password,
                    user.principal_key,
                    user.up_bps,
                    user.down_bps,
                )
            }),
        ))
    }

    fn new(protocol: crate::Socks5InboundTcpAcceptor) -> Self {
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
        E: From<zero_core::Error> + From<RuntimeError>,
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

impl<S> InboundClientResponse<S> for Socks5InboundAcceptor
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
    fn new(protocol: crate::udp::Socks5InboundUdpAssociationSession) -> Self {
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
