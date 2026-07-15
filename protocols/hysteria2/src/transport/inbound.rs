use std::path::{Path, PathBuf};

use zero_core::InboundClientResponse;
use zero_traits::AsyncSocket;
use zero_transport::RuntimeError;

use super::{
    options::{Hysteria2InboundBindOptionsRef, Hysteria2InboundOptionsRef},
    quic_alpn_protocols, Hysteria2AuthenticatedInboundProfile,
    Hysteria2AuthenticatedQuicConnection, Hysteria2InboundTcpResponseProtocol, Hysteria2Stream,
};

#[derive(Debug, Clone)]
pub struct Hysteria2InboundBindPlan {
    cert_path: String,
    key_path: String,
    source_dir: Option<PathBuf>,
}

impl Hysteria2InboundBindPlan {
    pub fn from_options_refs(
        source_dir: Option<&Path>,
        options: Hysteria2InboundBindOptionsRef<'_>,
    ) -> Self {
        Self::from_paths(source_dir, options.cert_path, options.key_path)
    }

    pub fn from_paths(
        source_dir: Option<&Path>,
        cert_path: Option<&str>,
        key_path: Option<&str>,
    ) -> Self {
        Self {
            cert_path: cert_path.unwrap_or("certs/fullchain.pem").to_owned(),
            key_path: key_path.unwrap_or("certs/privkey.pem").to_owned(),
            source_dir: source_dir.map(PathBuf::from),
        }
    }

    pub async fn bind(
        &self,
        listen_addr: &str,
    ) -> Result<zero_transport::quic::QuicInbound, RuntimeError> {
        let alpn_protocols = quic_alpn_protocols();
        zero_transport::quic::QuicInbound::bind(
            listen_addr,
            &self.cert_path,
            &self.key_path,
            self.source_dir.as_deref(),
            &alpn_protocols,
        )
        .await
    }
}

#[async_trait::async_trait]
impl zero_transport::inbound_route::ProtocolInboundBindPlan for Hysteria2InboundBindPlan {
    async fn bind(
        &self,
        listen_addr: &str,
    ) -> Result<zero_transport::inbound_route::TransportInboundBindTarget, RuntimeError> {
        Ok(
            zero_transport::inbound_route::TransportInboundBindTarget::Quic(
                Hysteria2InboundBindPlan::bind(self, listen_addr).await?,
            ),
        )
    }
}

pub fn inbound_profile_from_password(password: &str) -> Hysteria2AuthenticatedInboundProfile {
    Hysteria2AuthenticatedInboundProfile::new(crate::inbound::inbound_profile_from_config_password(
        password,
    ))
}

pub fn inbound_profile_from_options(
    options: Hysteria2InboundOptionsRef<'_>,
) -> Hysteria2AuthenticatedInboundProfile {
    inbound_profile_from_password(options.password)
}

pub fn inbound_tcp_acceptor() -> Hysteria2InboundTcpResponseProtocol {
    Hysteria2InboundTcpResponseProtocol {
        protocol: crate::inbound::Hysteria2InboundTcpAcceptor::new(),
    }
}

impl Hysteria2AuthenticatedInboundProfile {
    fn new(protocol: crate::inbound::Hysteria2InboundProfile) -> Self {
        Self { protocol }
    }

    pub fn tcp_response_protocol(&self) -> Hysteria2InboundTcpResponseProtocol {
        inbound_tcp_acceptor()
    }
}

impl<S> InboundClientResponse<S> for Hysteria2InboundTcpResponseProtocol
where
    S: AsyncSocket,
{
    async fn send_ok(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_ok(client).await
    }

    async fn send_blocked(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_blocked(client).await
    }

    async fn send_upstream_failure(&self, client: &mut S) -> Result<(), zero_core::Error> {
        self.protocol.send_upstream_failure(client).await
    }
}

#[async_trait::async_trait]
impl zero_transport::inbound_quic::AuthenticatedQuicInboundProfile
    for Hysteria2AuthenticatedInboundProfile
{
    type Connection = Hysteria2AuthenticatedQuicConnection;

    async fn accept_authenticated_connection(
        &self,
        connection: quinn::Connection,
    ) -> Result<Self::Connection, RuntimeError> {
        let protocol = self
            .protocol
            .accept_authenticated_quic_session(connection, Hysteria2Stream::new)
            .await
            .map_err(RuntimeError::from)?;
        Ok(Hysteria2AuthenticatedQuicConnection { protocol })
    }
}

#[async_trait::async_trait]
impl zero_transport::inbound_quic::AuthenticatedQuicInboundConnection
    for Hysteria2AuthenticatedQuicConnection
{
    type Stream = Hysteria2Stream;
    type ResponseProtocol = Hysteria2InboundTcpResponseProtocol;
    type UdpRelay = crate::udp::Hysteria2InboundUdpRelay;

    fn datagram_source(&self) -> std::sync::Arc<quinn::Connection> {
        self.protocol.connection()
    }

    fn udp_relay(&self) -> Self::UdpRelay {
        self.protocol.accept_udp_session()
    }

    fn response_protocol(&self) -> Self::ResponseProtocol {
        inbound_tcp_acceptor()
    }

    async fn accept_next_tcp_stream(
        &self,
    ) -> Result<Option<(zero_core::Session, Self::Stream)>, RuntimeError> {
        self.protocol
            .accept_next_tcp_stream(Hysteria2Stream::new)
            .await
            .map_err(RuntimeError::from)
    }
}
